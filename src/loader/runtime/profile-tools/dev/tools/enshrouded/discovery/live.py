"""Read-only Windows process evidence; no injection, writes, or engine calls."""
import ctypes as C
from ctypes import wintypes as W
import hashlib
from pathlib import Path
import struct
from collections import Counter


class MBI(C.Structure):
    _fields_ = [('BaseAddress', C.c_void_p), ('AllocationBase', C.c_void_p),
                ('AllocationProtect', W.DWORD), ('PartitionId', W.WORD),
                ('RegionSize', C.c_size_t), ('State', W.DWORD), ('Protect', W.DWORD), ('Type', W.DWORD)]


class Module(C.Structure):
    _fields_ = [('dwSize', W.DWORD), ('th32ModuleID', W.DWORD), ('th32ProcessID', W.DWORD),
                ('GlblcntUsage', W.DWORD), ('ProccntUsage', W.DWORD), ('modBaseAddr', C.c_void_p),
                ('modBaseSize', W.DWORD), ('hModule', W.HMODULE),
                ('szModule', W.WCHAR * 256), ('szExePath', W.WCHAR * 260)]


class Process:
    def __init__(self, pid, pe):
        if not hasattr(C, 'WinDLL'):
            raise ValueError('live capture requires Windows x64')
        self.k = C.WinDLL('kernel32', use_last_error=True)
        for name, args, result in (
            ('OpenProcess', [W.DWORD, W.BOOL, W.DWORD], W.HANDLE),
            ('CloseHandle', [W.HANDLE], W.BOOL),
            ('ReadProcessMemory', [W.HANDLE, C.c_void_p, C.c_void_p, C.c_size_t, C.POINTER(C.c_size_t)], W.BOOL),
            ('VirtualQueryEx', [W.HANDLE, C.c_void_p, C.POINTER(MBI), C.c_size_t], C.c_size_t),
            ('CreateToolhelp32Snapshot', [W.DWORD, W.DWORD], W.HANDLE),
            ('Module32FirstW', [W.HANDLE, C.POINTER(Module)], W.BOOL),
        ):
            fn = getattr(self.k, name)
            fn.argtypes, fn.restype = args, result
        self.handle = self.k.OpenProcess(0x400 | 0x10, False, pid)
        if not self.handle:
            raise C.WinError(C.get_last_error())
        try:
            snap = self.k.CreateToolhelp32Snapshot(0x8, pid)
            if snap == C.c_void_p(-1).value:
                raise C.WinError(C.get_last_error())
            try:
                module = Module(dwSize=C.sizeof(Module))
                if not self.k.Module32FirstW(snap, C.byref(module)):
                    raise C.WinError(C.get_last_error())
            finally:
                self.k.CloseHandle(snap)
            actual = Path(module.szExePath)
            if not actual.samefile(pe.path) or hashlib.sha256(actual.read_bytes()).hexdigest() != pe.sha256:
                raise ValueError('PID executable does not match the selected executable SHA-256/path')
            self.base = module.modBaseAddr
            self.size = module.modBaseSize
            headers = self.read(self.base, 4096)
            nt = struct.unpack_from('<I', headers, 0x3c)[0]
            if struct.unpack_from('<I', headers, nt + 8)[0] != pe.timestamp or self.size != pe.image_size:
                raise ValueError('loaded PE identity mismatch')
        except Exception:
            self.close()
            raise

    def close(self):
        if self.handle:
            self.k.CloseHandle(self.handle)
            self.handle = None

    def read(self, address, size):
        data = C.create_string_buffer(size)
        received = C.c_size_t()
        if not self.k.ReadProcessMemory(self.handle, address, data, size, C.byref(received)) or received.value != size:
            raise OSError(f'ReadProcessMemory failed at 0x{address:x}+{size}: {C.get_last_error()}')
        return data.raw

    def regions(self):
        address = 0
        while True:
            mbi = MBI()
            if self.k.VirtualQueryEx(self.handle, address, C.byref(mbi), C.sizeof(mbi)) != C.sizeof(mbi):
                break
            begin = mbi.BaseAddress or 0
            end = begin + mbi.RegionSize
            if end <= address:
                raise ValueError('non-progressing VirtualQueryEx')
            if mbi.State == 0x1000 and not mbi.Protect & (0x100 | 1) and mbi.Protect & 0xee:
                yield begin, mbi.RegionSize, mbi.Type, mbi.Protect
            address = end


def capture(pe, reflection, pid, profile=None, progress=print):
    import numpy as np
    process = Process(pid, pe)
    try:
        types = reflection['types']
        addresses = {process.base + t['metadataRva']: t for t in types}
        ordered = np.array(sorted(addresses), dtype='<u8')
        descriptor_min, descriptor_max = int(ordered[0]), int(ordered[-1])
        # Validate every live descriptor and every static registry slot before joining memory pointers.
        verified, mismatches = [], []
        for t in types:
            rva = t['metadataRva']
            try:
                live = process.read(process.base + rva + 0x40, 24)
                slot = struct.unpack('<Q', process.read(process.base + t['registrySlotRva'], 8))[0]
                if live != pe.read(rva + 0x40, 24) or slot != process.base + rva:
                    raise ValueError('descriptor header or registry pointer changed')
                verified.append(t['index'])
            except (OSError, ValueError) as error:
                mismatches.append(dict(typeIndex=t['index'], reason=str(error)))
        if mismatches:
            raise ValueError(f'{len(mismatches)} live reflection mismatches; refusing pointer joins')
        references, regions, skipped = [], [], []
        read_bytes = 0
        for begin, size, kind, protection in process.regions():
            regions.append(dict(address=hex(begin), size=size, kind=kind, protection=protection))
            for offset in range(0, size, 4 * 1024 * 1024):
                address = begin + offset
                wanted = min(4 * 1024 * 1024, size - offset)
                try:
                    data = process.read(address, wanted)
                    chunks = [(address, data)]
                except OSError:
                    chunks = []
                    # A racing/unreadable page must not hide the rest of a 4 MiB chunk.
                    for local in range(0, wanted, 4096):
                        try:
                            chunks.append((address + local, process.read(address + local, min(4096, wanted - local))))
                        except OSError:
                            skipped.append(dict(address=hex(address + local), size=min(4096, wanted - local)))
                for start, data in chunks:
                    read_bytes += len(data)
                    prefix = (-start) % 8
                    values = np.frombuffer(data, dtype='<u8', count=(len(data) - prefix) // 8, offset=prefix)
                    offsets = np.flatnonzero((values >= descriptor_min) & (values <= descriptor_max))
                    possible = values[offsets]
                    slots = np.searchsorted(ordered, possible)
                    hits = offsets[ordered[slots] == possible]
                    for slot in hits.tolist():
                        references.append((start + prefix + slot * 8, addresses[int(values[slot])]['index']))
        progress(f'live PID {pid}: read {read_bytes // (1024 * 1024)} MiB, {len(references)} metadata references')
        references.sort()
        runs, run = [], []
        for entry in references:
            if run and entry[0] != run[-1][0] + 8:
                if len(run) >= 4:
                    runs.append(run)
                run = []
            run.append(entry)
        if len(run) >= 4:
            runs.append(run)
        profile_rows = {row['name']: row for row in (profile or {}).get('components', [])}
        tables = []
        allocated = set()
        # Keen's observed arena stores a byte length immediately before this pointer array.
        # Discover this shape without any existing component index/name anchors. Treat its
        # role as a candidate until code/layout evidence establishes component semantics.
        for run in runs:
            begin = run[0][0]
            if process.base <= begin < process.base + process.size:
                continue
            try:
                byte_count = struct.unpack('<Q', process.read(begin - 8, 8))[0]
                if not 16 * 8 <= byte_count <= 4096 * 8 or byte_count % 8:
                    continue
                raw = process.read(begin, byte_count)
            except OSError:
                continue
            values = [v for (v,) in struct.iter_unpack('<Q', raw)]
            if any(v and (v not in addresses or not addresses[v]['qualifiedName'].startswith('keen::ecs::')) for v in values):
                continue
            nonnull = [v for v in values if v]
            if len(nonnull) < 16 or len(set(nonnull)) != len(nonnull):
                continue
            rows = [dict(index=i, typeIndex=addresses[v]['index'], name=addresses[v]['qualifiedName'],
                         size=addresses[v]['size'], qualifiedHash=addresses[v]['qualifiedHash'])
                    for i, v in enumerate(values) if v]
            conflicts = [dict(index=r['index'], name=r['name'], profile=profile_rows[r['name']])
                         for r in rows if r['name'] in profile_rows and
                         (profile_rows[r['name']]['index'] != r['index'] or profile_rows[r['name']]['size'] != r['size'])]
            matches = sum(r['name'] in profile_rows and profile_rows[r['name']]['index'] == r['index']
                          and profile_rows[r['name']]['size'] == r['size'] for r in rows)
            table = dict(address=hex(begin), count=len(rows), inspectedSlots=len(values),
                         allocationByteLength=byte_count, rows=rows, ecsTypes=len(rows),
                         nullSlots=[i for i, v in enumerate(values) if not v],
                         profileMatches=matches, profileConflicts=conflicts,
                         discovery='allocation-length-and-reflected-pointers', boundsProven=False,
                         status='allocated-ecs-table-candidate')
            if matches >= 4 and not conflicts:
                table['status'] = 'profile-anchored-component-table'
            tables.append(table)
            allocated.add(begin)
        for run in runs:
            begin = run[0][0]
            rows = [dict(index=i, typeIndex=type_index, name=types[type_index]['qualifiedName'],
                         size=types[type_index]['size'], qualifiedHash=types[type_index]['qualifiedHash'])
                    for i, (_, type_index) in enumerate(run)]
            matches = sum(profile_rows[r['name']]['index'] == r['index'] and profile_rows[r['name']]['size'] == r['size']
                          for r in rows if r['name'] in profile_rows)
            conflicts = [dict(index=r['index'], name=r['name'], profile=profile_rows[r['name']])
                         for r in rows if r['name'] in profile_rows and
                         (profile_rows[r['name']]['index'] != r['index'] or profile_rows[r['name']]['size'] != r['size'])]
            ecs = sum(r['name'].startswith('keen::ecs::') for r in rows)
            # Consecutive pointers alone do not establish an engine registry's role or bounds.
            table = dict(address=hex(begin), count=len(rows), ecsTypes=ecs, profileMatches=matches,
                         profileConflicts=conflicts, rows=rows, status='pointer-run-candidate')
            if process.base <= begin < process.base + process.size:
                table['rva'] = begin - process.base
            if begin == process.base + reflection['tableRva'] and len(rows) == reflection['count']:
                table['status'] = 'reflection-registry'
            elif len(rows) >= 16 and ecs == len(rows) and matches >= 4 and not conflicts:
                table['status'] = 'profile-anchored-component-table'
            tables.append(table)
        # Runtime component slots may contain holes; contiguous pointer runs cannot find these.
        votes = Counter()
        for address, type_index in references:
            known = profile_rows.get(types[type_index]['qualifiedName'])
            if known and known['size'] == types[type_index]['size']:
                votes[address - known['index'] * 8] += 1
        for begin, vote_count in votes.most_common():
            if vote_count < max(4, len(profile_rows) // 10):
                break
            if begin in allocated:
                continue
            rows, unknown_slots, conflicts = [], [], []
            # This is the provider's existing maximum, not an inferred registry count.
            capacity = 1024
            try:
                raw = process.read(begin, capacity * 8)
            except OSError:
                continue
            for index, (pointer,) in enumerate(struct.iter_unpack('<Q', raw)):
                ty = addresses.get(pointer)
                if ty is None:
                    if pointer:
                        unknown_slots.append(dict(index=index, value=hex(pointer)))
                    continue
                row = dict(index=index, typeIndex=ty['index'], name=ty['qualifiedName'],
                           size=ty['size'], qualifiedHash=ty['qualifiedHash'])
                rows.append(row)
                known = profile_rows.get(row['name'])
                if known and (known['index'] != index or known['size'] != row['size']):
                    conflicts.append(dict(index=index, name=row['name'], profile=known))
            table = dict(address=hex(begin), count=len(rows), inspectedSlots=capacity, rows=rows,
                         unknownSlots=unknown_slots, profileMatches=vote_count, profileConflicts=conflicts,
                         boundsProven=False, ecsTypes=sum(r['name'].startswith('keen::ecs::') for r in rows),
                         status='profile-anchored-component-table' if not conflicts else 'conflicting-component-table')
            if process.base <= begin < process.base + process.size:
                table['rva'] = begin - process.base
            tables.append(table)
        # Re-read candidates to expose tables modified during capture, not silently bless them.
        for table in tables:
            try:
                table['stableReadback'] = all(
                    process.read(int(table['address'], 16) + r['index'] * 8, 8) ==
                    struct.pack('<Q', process.base + types[r['typeIndex']]['metadataRva']) for r in table['rows'])
            except OSError:
                table['stableReadback'] = False
            if not table['stableReadback']:
                table['status'] = 'changed-during-capture'
        return dict(pid=pid, moduleBase=hex(process.base), image=pe.identity(),
                    verifiedReflectionTypes=len(verified), bytesRead=read_bytes, skippedPages=skipped,
                    regions=regions, tables=tables, metadataReferenceCount=len(references),
                    consistentSnapshot=False,
                    limitations=['running process was not suspended', 'pointer runs do not prove table bounds or function ABI'])
    finally:
        process.close()
