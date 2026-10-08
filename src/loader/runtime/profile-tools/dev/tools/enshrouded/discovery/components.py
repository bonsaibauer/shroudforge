"""Resolve component registration, storage, and template layouts separately.

Layout v1 is checked against every reflected descriptor and both parallel arrays.
No component profile, known name list, or fixed address participates in discovery.
"""
import struct
from collections import Counter


def pointer_references(process, targets):
    import numpy as np
    targets = np.array(sorted(targets), dtype='<u8')
    if not len(targets):
        return
    for begin, size, _, _ in process.regions():
        for offset in range(0, size, 4 * 1024 * 1024):
            address = begin + offset
            try:
                data = process.read(address, min(4 * 1024 * 1024, size - offset))
            except OSError:
                continue
            values = np.frombuffer(data[:len(data) // 8 * 8], dtype='<u8')
            slots = np.searchsorted(targets, values)
            valid = np.flatnonzero(slots < len(targets))
            for slot in valid[targets[slots[valid]] == values[valid]].tolist():
                yield address + slot * 8, int(values[slot])


def read_registry(process, pe, reflection, owner):
    def u64(address):
        return struct.unpack('<Q', process.read(address, 8))[0]

    records, count, capacity = struct.unpack('<QQQ', process.read(owner + 8, 24))
    sizes, sizes_count = struct.unpack('<QQ', process.read(owner + 232, 16))
    types, types_count = struct.unpack('<QQ', process.read(owner + 256, 16))
    callbacks, callbacks_count = struct.unpack('<QQ', process.read(owner + 280, 16))
    if not 16 <= count <= 1024 or not count <= capacity <= 4096 or any(
            n != count for n in (sizes_count, types_count, callbacks_count)):
        raise ValueError('component registry v1 array headers mismatch')
    descriptors = {process.base + t['metadataRva']: t for t in reflection['types']}
    by_name = {t['qualifiedName']: t for t in reflection['types']}
    raw = process.read(records, count * 256)
    storage_sizes = struct.unpack('<' + 'H' * count, process.read(sizes, count * 2))
    storage_types = struct.unpack('<' + 'Q' * count, process.read(types, count * 8))
    callback_slots = struct.unpack('<' + 'Q' * count * 5, process.read(callbacks, count * 40))
    rows, seen, runtime_seen = [], set(), set()
    for i in range(count):
        _, _, name, length, qhash, runtime, template = struct.unpack_from('<7Q', raw, i * 256)
        size, storage_flags, flags = struct.unpack_from('<HHI', raw, i * 256 + 56)
        if not 1 <= length <= 512:
            raise ValueError('invalid registration name length')
        # Names are EXE-backed; the record spans include a trailing zero.
        name = pe.read(name - process.base, length).rstrip(b'\0').decode('utf-8')
        identity = by_name.get(name)
        rt, config = descriptors.get(runtime), descriptors.get(template)
        if (not name.startswith('keen::ecs::') or not identity or qhash != identity['qualifiedHash']
                or name in seen or (runtime and not rt) or (template and not config)
                or not (rt or config) or (config or rt)['qualifiedName'] != name
                or size != (rt['size'] if rt else 0)
                or runtime != storage_types[i] or size != storage_sizes[i]):
            raise ValueError(f'registration {i} identity/storage mismatch')
        if rt and rt['index'] in runtime_seen:
            raise ValueError('multiple registrations for one runtime layout')
        seen.add(name)
        if rt:
            runtime_seen.add(rt['index'])
        row = dict(index=i, qualifiedName=name, qualifiedHash=qhash,
                   runtimeType=rt['index'] if rt else None,
                   templateType=config['index'] if config else None,
                   runtimeSize=size, flagsBits=flags, storageFlagsBits=storage_flags,
                   storage='entity' if rt else 'template-only', callbacks=[])
        for slot in range(4):
            pointer = callback_slots[i * 5 + slot]
            if pointer:
                rva = pointer - process.base
                if not pe.executable(rva):
                    raise ValueError('component callback outside executable code')
                row['callbacks'].append(dict(slotOffset=slot * 8, functionRva=rva,
                                              origin='parallel-callback-table', abi=None, callable=False))
        for offset in range(88, 256, 8):
            pointer = struct.unpack_from('<Q', raw, i * 256 + offset)[0]
            rva = pointer - process.base
            if pe.executable(rva):
                row['callbacks'].append(dict(slotOffset=offset, functionRva=rva,
                                              origin='registration-record', abi=None, callable=False))
        rows.append(row)
    again = process.read(records, len(raw))
    if any(raw[i * 256:i * 256 + 88] != again[i * 256:i * 256 + 88] for i in range(count)):
        raise ValueError('registration changed during capture')
    if u64(owner + 8) != records or u64(owner + 16) != count:
        raise ValueError('registry array changed during capture')
    if (process.read(sizes, count * 2) != struct.pack('<' + 'H' * count, *storage_sizes)
            or process.read(types, count * 8) != struct.pack('<' + 'Q' * count, *storage_types)):
        raise ValueError('parallel storage arrays changed during capture')
    return dict(ownerAddress=hex(owner), recordsAddress=hex(records), count=count,
                layoutVersion=1, recordStride=256, stableReadback=True, rows=rows,
                runtimeTypes=sum(row['runtimeType'] is not None for row in rows),
                templateTypes=sum(row['templateType'] is not None for row in rows),
                templateOnly=sum(row['storage'] == 'template-only' for row in rows))


def validate_manager(process, manager, registry, limit=4096):
    """Independently connect the registration index to live archetype strides."""
    def u64(address):
        return struct.unpack('<Q', process.read(address, 8))[0]
    if u64(manager) != int(registry['ownerAddress'], 16):
        raise ValueError('manager owner mismatch')
    count, table = u64(manager + 344), u64(manager + 392)
    if not 1 <= count <= 1 << 20:
        raise ValueError('invalid manager table length')
    pointers = struct.unpack('<' + 'Q' * min(count, limit), process.read(table, min(count, limit) * 8))
    layouts, entities, slots = set(), set(), Counter()
    for entity in pointers:
        if not entity or entity in entities:
            continue
        try:
            eid, generation, layout, storage = struct.unpack('<IIQQ', process.read(entity + 16, 24))
            if not eid or not layout or not storage or layout in layouts:
                continue
            n = registry['count']
            bits = int.from_bytes(process.read(layout, (n + 63) // 64 * 8), 'little')
            strides = struct.unpack('<' + 'H' * n, process.read(layout + 2692, n * 2))
            current = [i for i in range(n) if bits & (1 << i)]
            if not current or any(strides[i] != registry['rows'][i]['runtimeSize']
                                  or not strides[i] for i in current):
                continue
            layouts.add(layout)
            entities.add(entity)
            slots.update(current)
        except OSError:
            continue
    if len(layouts) < 2 or len(slots) < 4:
        raise ValueError('insufficient independent entity/archetype evidence')
    return dict(address=hex(manager), sampledTableSlots=len(pointers),
                distinctLayouts=len(layouts), confirmedComponentIndices=sorted(slots),
                method='entity-layout-bits-and-strides', consistentSnapshot=False)


def discover(process, pe, reflection, live, progress=print):
    candidates = {int(t['address'], 16) for t in live['tables']
                  if t.get('allocationByteLength') and t['stableReadback']}
    registries = {}
    for address, target in pointer_references(process, candidates):
        owner = address - 256
        try:
            registry = read_registry(process, pe, reflection, owner)
            registries[owner] = registry
        except (OSError, ValueError, UnicodeError):
            continue
    progress(f'full component registries: {len(registries)}')
    for registry in registries.values():
        registry['managers'] = []
    for address, owner in pointer_references(process, registries):
        try:
            registries[owner]['managers'].append(validate_manager(process, address, registries[owner]))
        except (OSError, ValueError):
            continue
    return dict(schemaVersion=1, image=pe.identity(), pid=live['pid'],
                moduleBase=hex(process.base), profileIndependent=True,
                registries=list(registries.values()),
                limitations=['structure layout v1 is validated, not universal across future builds',
                             'callback ABIs and effects remain unknown; no callbacks were invoked',
                             'template-only data is not an entity storage column'])
