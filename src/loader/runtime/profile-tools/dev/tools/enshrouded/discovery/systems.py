"""Read named ECS execution descriptors, preserving engine spellings and raw slots."""
import re


def discover(pe, functions):
    roots = {f['beginRva'] for f in functions['functionGroups']}

    def span(pointer, length):
        if not 1 <= length <= 256:
            raise ValueError('invalid string span')
        value = pe.read(pe.rva(pointer), length).rstrip(b'\0')
        if not value or any(c < 32 or c >= 127 for c in value):
            raise ValueError('invalid descriptor string')
        return value.decode('ascii')

    records = {}
    for pointer in functions['codePointers']:
        root, slot = pointer['targetRva'], pointer['slotRva']
        if root not in roots or slot < 16:
            continue
        try:
            name = span(*pe.u('<QQ', slot - 16))
            if not re.fullmatch(r'[a-z][a-z0-9_]{2,95}', name):
                continue
            arrays = []
            for offset in (24, 40, 56, 72, 88):
                data, count = pe.u('<QQ', slot - 16 + offset)
                if count > 256 or bool(data) != bool(count):
                    raise ValueError('invalid descriptor array')
                values = [span(*pe.u('<QQ', pe.rva(data) + i * 16)) for i in range(count)]
                arrays.append(dict(offset=offset, entries=values))
            if not any(a['entries'] for a in arrays):
                continue
            records[slot - 16] = dict(name=name, functionRva=root, descriptorRva=slot - 16,
                                     nameSource='engine-execution-descriptor', dependencySlots=arrays)
        except (ValueError, UnicodeError):
            continue
    return sorted(records.values(), key=lambda r: r['descriptorRva'])
