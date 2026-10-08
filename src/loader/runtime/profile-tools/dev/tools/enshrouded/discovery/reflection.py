"""Extract the same registry as kfc-base, retaining binary addresses and all hashes.

Layout source: kfc-base/src/reflection/extract/parser.rs. No profile or cached
reflection is used to discover the table. Hashes are read from metadata.
"""
import struct

PRIMITIVES = ('NONE BOOL UINT8 SINT8 UINT16 SINT16 UINT32 SINT32 UINT64 SINT64 '
              'FLOAT32 FLOAT64 ENUM BITMASK8 BITMASK16 BITMASK32 BITMASK64 TYPEDEF '
              'STRUCT STATIC_ARRAY DS_ARRAY DS_STRING DS_OPTIONAL DS_VARIANT '
              'BLOB_ARRAY BLOB_STRING BLOB_OPTIONAL BLOB_VARIANT OBJECT_REFERENCE GUID').split()


def fnv(text):
    value = 0x811c9dc5
    for byte in text.encode('utf-8'):
        value = ((value ^ byte) * 0x1000193) & 0xffffffff
    return value


def find_registry(pe):
    anchors = []
    for name in (b'BlobString', b'uint32'):
        descriptors = []
        for text_rva in pe.find(b'\0' + name + b'\0'):
            descriptors.extend(pe.find(struct.pack('<QQ', pe.base + text_rva + 1, len(name)), 8))
        anchors.append(descriptors)
    candidates = set()
    for first in anchors[0]:
        for second in anchors[1]:
            for table in pe.find(struct.pack('<QQ', pe.base + first, pe.base + second), 8):
                for header in pe.find(struct.pack('<Q', pe.base + table), 8):
                    count = pe.u('<Q', header + 8)[0]
                    if not 2 <= count <= 100000:
                        continue
                    try:
                        pointers = pe.u('<' + 'Q' * count, table)
                        if len(set(pointers)) != count:
                            continue
                        for pointer in pointers:
                            pe.read(pe.rva(pointer), 0x90)
                    except ValueError:
                        continue
                    candidates.add((header, table, count))
    tables = {(table, count) for _, table, count in candidates}
    if len(tables) != 1:
        raise ValueError(f'expected one reflection registry, found {len(tables)}: {sorted(candidates)}')
    table, count = next(iter(tables))
    return sorted(header for header, _, _ in candidates), table, count


def extract(pe):
    headers, table, count = find_registry(pe)
    pointers = pe.u('<' + 'Q' * count, table)
    references = {va: i for i, va in enumerate(pointers)}
    attribute_duplicates = []

    def ref(va):
        if va == 0:
            return None
        if va not in references:
            raise ValueError(f'unresolved reflected type pointer: 0x{va:x}')
        return references[va]

    def namespace(va):
        names, seen = [], set()
        while va:
            if va in seen or len(seen) > 128:
                raise ValueError('cyclic namespace')
            seen.add(va)
            text, length, va = pe.u('<QQQ', pe.rva(va))
            names.append(pe.text(text, length))
        return list(reversed(names))

    def attributes(va, length):
        if length > 10000 or (length and not va):
            raise ValueError('invalid attribute count/pointer')
        result = {}
        for i in range(length):
            metadata, value, value_len = pe.u('<QQQ', pe.rva(va) + i * 24)
            ns, name, name_len, ty = pe.u('<QQQQ', pe.rva(metadata))
            name = pe.text(name, name_len)
            if name in result:
                attribute_duplicates.append(dict(arrayRva=pe.rva(va), ordinal=i,
                                                 previous=result[name]))
            result[name] = dict(name=name, namespace=namespace(ns), type=ref(ty), value=pe.text(value, value_len))
        return result

    types = []
    for index, pointer in enumerate(pointers):
        rva = pe.rva(pointer)
        name, name_len, impact, impact_len, qualified, qualified_len, ns, inner = pe.u('<8Q', rva)
        size, alignment, element_alignment, field_count, primitive, flags, qhash, ihash = pe.u('<IHHIBB2xII', rva + 0x40)
        if primitive >= len(PRIMITIVES) or field_count > 100000:
            raise ValueError('invalid reflected type header')
        fields, enums, variant, default, default_len, attrs, attr_count = pe.u('<7Q', rva + 0x58)
        name, impact, qualified = pe.text(name, name_len), pe.text(impact, impact_len), pe.text(qualified, qualified_len)
        record = dict(index=index, name=name, impactName=impact, qualifiedName=qualified,
                      namespace=namespace(ns), innerType=ref(inner), size=size, alignment=alignment,
                      elementAlignment=element_alignment, fieldCount=field_count,
                      primitiveType=PRIMITIVES[primitive], flagsBits=flags,
                      nameHash=fnv(name), impactHash=fnv(impact), qualifiedHash=qhash, internalHash=ihash,
                      metadataRva=rva, registrySlotRva=table + index * 8,
                      structFields={}, enumFields={}, attributes=attributes(attrs, attr_count))
        if default_len > 16 * 1024 * 1024:
            raise ValueError('oversized default value')
        if default:
            record['defaultValue'] = list(pe.read(pe.rva(default), default_len))
        if variant:
            record['variantTypesRva'] = pe.rva(variant)
        if fields:
            for i in range(field_count):
                text, length, ty, offset, attr, attr_len = pe.u('<6Q', pe.rva(fields) + i * 48)
                text = pe.text(text, length)
                if text in record['structFields'] or ref(ty) is None:
                    raise ValueError('invalid/duplicate reflected field')
                record['structFields'][text] = dict(name=text, type=ref(ty), dataOffset=offset,
                                                    attributes=attributes(attr, attr_len))
        if enums:
            for i in range(field_count):
                text, length, value = pe.u('<QQQ', pe.rva(enums) + i * 40)
                text = pe.text(text, length)
                if text in record['enumFields']:
                    raise ValueError('duplicate reflected enum member')
                record['enumFields'][text] = dict(name=text, value=value)
        types.append(record)
    collisions = {}
    for kind in ('qualifiedHash', 'internalHash', 'nameHash', 'impactHash'):
        buckets = {}
        for ty in types:
            buckets.setdefault(str(ty[kind]), []).append(ty['index'])
        collisions[kind] = {key: value for key, value in buckets.items() if len(value) > 1}
    return dict(headerRvas=headers, tableRva=table, count=count, types=types,
                hashCollisions=collisions, attributeDuplicates=attribute_duplicates)
