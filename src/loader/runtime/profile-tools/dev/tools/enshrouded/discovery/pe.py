"""Bounds-checked PE32+ view. Addresses in artifacts are RVAs, never ASLR VAs."""
import bisect
import hashlib
import struct
from pathlib import Path


class PE:
    def __init__(self, path):
        self.path = Path(path).resolve()
        self.data = self.path.read_bytes()
        if self.data[:2] != b'MZ':
            raise ValueError('not an MZ executable')
        nt = self.unpack('<I', 0x3c)[0]
        if self.data[nt:nt + 4] != b'PE\0\0':
            raise ValueError('not a PE executable')
        machine, count, self.timestamp, _, _, optional_size, _ = self.unpack('<HHIIIHH', nt + 4)
        opt = nt + 24
        if machine != 0x8664 or self.unpack('<H', opt)[0] != 0x20b or optional_size < 240:
            raise ValueError('expected Windows x64 PE32+')
        self.base = self.unpack('<Q', opt + 24)[0]
        self.image_size, self.headers_size = self.unpack('<II', opt + 56)
        if not 4096 <= self.image_size <= 1 << 30:
            raise ValueError('invalid image size')
        self.sha256 = hashlib.sha256(self.data).hexdigest()
        self.sections = []
        for i in range(count):
            name, size, rva, raw_size, raw, _, _, _, _, flags = self.unpack('<8sIIIIIIHHI', opt + optional_size + i * 40)
            if raw + raw_size > len(self.data) or rva + size > self.image_size:
                raise ValueError('section outside file/image')
            self.sections.append(dict(name=name.rstrip(b'\0').decode('ascii'), rva=rva,
                                      size=size, raw=raw, rawSize=raw_size, flags=flags))
        self.functions = []
        exception_rva, exception_size = self.unpack('<II', opt + 112 + 3 * 8)
        if exception_rva:
            if exception_size % 12:
                raise ValueError('truncated exception directory')
            for begin, end, unwind in struct.iter_unpack('<III', self.read(exception_rva, exception_size)):
                if not begin < end <= self.image_size or not self.executable(begin):
                    raise ValueError('invalid RUNTIME_FUNCTION range')
                self.functions.append(dict(beginRva=begin, endRva=end, unwindRva=unwind))
        self.functions.sort(key=lambda f: f['beginRva'])
        self.function_starts = [f['beginRva'] for f in self.functions]
        self.unknown_unwind_versions = []
        for function in self.functions:
            root, unwind = function['beginRva'], function['unwindRva']
            seen = set()
            while True:
                if unwind in seen or len(seen) > 128:
                    raise ValueError('cyclic/oversized chained unwind information')
                seen.add(unwind)
                version_flags, _, codes, _ = self.u('<BBBB', unwind)
                version, flags = version_flags & 7, version_flags >> 3
                if version not in (1, 2):
                    self.unknown_unwind_versions.append(dict(beginRva=function['beginRva'], version=version))
                    root = None
                    break
                if not flags & 4:
                    break
                if flags & 3:
                    raise ValueError('chained unwind info also declares an exception handler')
                parent, parent_end, unwind = self.u('<III', unwind + 4 + ((codes + 1) & ~1) * 2)
                if not parent < parent_end <= self.image_size or not self.executable(parent):
                    raise ValueError('invalid chained RUNTIME_FUNCTION')
                root = parent
            function['primaryBeginRva'] = root

    def unpack(self, fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(self.data):
            raise ValueError('truncated PE structure')
        return struct.unpack_from(fmt, self.data, offset)

    def raw(self, rva, size=1):
        if rva >= 0 and rva + size <= min(self.headers_size, len(self.data)):
            return rva
        for section in self.sections:
            delta = rva - section['rva']
            if 0 <= delta and delta + size <= section['rawSize']:
                return section['raw'] + delta
        raise ValueError(f'RVA 0x{rva:x}+{size} has no file-backed bytes')

    def read(self, rva, size):
        offset = self.raw(rva, size)
        return self.data[offset:offset + size]

    def u(self, fmt, rva):
        return struct.unpack(fmt, self.read(rva, struct.calcsize(fmt)))

    def rva(self, va):
        rva = va - self.base
        if not 0 <= rva < self.image_size:
            raise ValueError(f'VA 0x{va:x} outside preferred image')
        return rva

    def text(self, va, length):
        if not 0 <= length <= 65536:
            raise ValueError('invalid reflection string length')
        return self.read(self.rva(va), length).decode('utf-8') if length else ''

    def executable(self, rva):
        return any(s['rva'] <= rva < s['rva'] + s['size'] and s['flags'] & 0x20000000
                   for s in self.sections)

    def find(self, needle, aligned=1, executable=False):
        result = []
        for s in self.sections:
            if bool(s['flags'] & 0x20000000) != executable:
                continue
            chunk = self.data[s['raw']:s['raw'] + s['rawSize']]
            offset = chunk.find(needle)
            while offset >= 0:
                rva = s['rva'] + offset
                if rva % aligned == 0:
                    result.append(rva)
                offset = chunk.find(needle, offset + 1)
        return result

    def containing_function(self, rva):
        index = bisect.bisect_right(self.function_starts, rva) - 1
        if index >= 0 and rva < self.functions[index]['endRva']:
            return index
        return None

    def identity(self):
        return dict(target=self.path.name, sha256=self.sha256, timestamp=self.timestamp,
                    size=self.image_size)
