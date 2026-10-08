"""Instruction-boundary xrefs, direct calls, and explicit unresolved semantics."""
import bisect
import hashlib
import re
import json
import struct


def mask_rip_displacement(code, instruction, begin, displacement):
    # In 64-bit address mode RIP-relative displacements are always signed disp32.
    # Capstone 5.0.7 reports disp_size=2 for e.g. 66 0f 6f 0d <disp32> (movdqa).
    # Validate against decoded bytes instead of trusting that encoding-size field.
    offset = instruction.address - begin + instruction.disp_offset
    if (instruction.disp_offset + 4 > instruction.size or
            struct.unpack_from('<i', code, offset)[0] != displacement):
        raise ValueError(f'inconsistent RIP displacement decoding at RVA 0x{instruction.address:x}')
    code[offset:offset + 4] = b'\0' * 4


def group_functions(pe, functions):
    """Join UNW_FLAG_CHAININFO fragments before comparing whole native procedures.

    https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64
    """
    rows = {f['beginRva']: f for f in functions}
    groups = {}
    for native in pe.functions:
        root = native['primaryBeginRva']
        rows[native['beginRva']]['primaryBeginRva'] = root
        if root is not None:
            groups.setdefault(root, []).append(rows[native['beginRva']])
    result = []
    for root, members in sorted(groups.items()):
        members.sort(key=lambda f: f['beginRva'])
        digests = [(m['endRva'] - m['beginRva'], m.get('relocatedCodeSha256')) for m in members]
        digest = hashlib.sha256(json.dumps(digests).encode('ascii')).hexdigest() if all(d for _, d in digests) else None
        result.append(dict(beginRva=root, ranges=[m['beginRva'] for m in members],
                           codeBytes=sum(length for length, _ in digests),
                           relocatedCodeSha256=digest, semanticStatus='unverified', callable=False))
    return result


def profile_audit(pe, profile):
    if profile is None:
        return None
    image = profile['image']
    identity_matches = (profile['target'].lower() == pe.path.name.lower() and
                        image['timestamp'] == pe.timestamp and image['size'] == pe.image_size and
                        (not image.get('sha256') or image['sha256'] == pe.sha256))
    records = []

    def scan(pattern):
        tokens = pattern.split()
        if not tokens:
            return []
        needle = b''.join(b'.' if t in ('?', '??') else re.escape(bytes([int(t, 16)])) for t in tokens)
        # Lookahead retains overlapping occurrences; DOTALL includes 0x0a wildcards.
        regex = re.compile(b'(?=(' + needle + b'))', re.DOTALL)
        return [section['rva'] + match.start() for section in pe.sections if section['flags'] & 0x20000000
                for match in regex.finditer(pe.read(section['rva'], section['rawSize']))]

    for name, hook in profile.get('hooks', {}).items():
        matches = scan(hook['signature'])
        records.append(dict(id='hook.' + name, kind='hook', matches=matches,
                            status='unique-byte-match' if len(matches) == 1 else 'unresolved'))
    for name, operation in profile.get('worldOperations', {}).items():
        guard = operation.get('guardBytes', [])
        rva = operation.get('functionRva')
        guard_rva = operation.get('guardRva', rva)
        try:
            matches = bool(guard) and guard_rva is not None and pe.read(guard_rva, len(guard)) == bytes(guard)
        except ValueError:
            matches = False
        row = dict(id=name, kind='world-operation', functionRva=rva, guardRva=guard_rva,
                   guardMatches=matches, abi=operation.get('abi'), thread=operation.get('thread'),
                   status='guard-match' if identity_matches and matches else 'unresolved')
        if rva is not None:
            idx = pe.containing_function(rva)
            if idx is not None:
                row['containingFunctionRva'] = pe.functions[idx]['beginRva']
                row['offsetWithinFunction'] = rva - pe.functions[idx]['beginRva']
        records.append(row)
    for name, patch in profile.get('runtimePatches', {}).items():
        matches = scan(patch['signature'])
        records.append(dict(id=name, kind='patch', matches=matches,
                            status='unique-byte-match' if len(matches) == 1 else 'unresolved'))
    return dict(id=profile['id'], imageMatches=identity_matches,
                identityStrength='sha256' if image.get('sha256') else 'timestamp-and-size',
                operations=records, semanticsVerifiedByDiscovery=False)


def discover(pe, reflection, progress=print):
    import capstone as cs
    from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_REG_RIP
    import numpy as np

    engine = cs.Cs(cs.CS_ARCH_X86, cs.CS_MODE_64)
    engine.detail = True
    types = reflection['types']
    metadata = {t['metadataRva']: t['index'] for t in types}
    metadata_starts = sorted(metadata)
    metadata_slots = {}
    for section in pe.sections:
        if section['flags'] & 0x20000000:
            continue
        prefix = (-section['rva']) % 8
        data = pe.read(section['rva'], section['rawSize'])
        values = np.frombuffer(data, dtype='<u8', offset=prefix, count=(len(data) - prefix) // 8)
        candidates = np.flatnonzero((values >= pe.base + metadata_starts[0]) &
                                   (values <= pe.base + metadata_starts[-1]))
        for slot in candidates.tolist():
            ty = metadata.get(int(values[slot]) - pe.base)
            if ty is not None:
                metadata_slots[section['rva'] + prefix + slot * 8] = ty
    hash_types = {}
    for t in types:
        for kind in ('qualifiedHash', 'internalHash', 'nameHash', 'impactHash'):
            hash_types.setdefault(t[kind], []).append(dict(typeIndex=t['index'], hashKind=kind))
    functions, undecoded = [], []
    decoded_bytes = 0
    for ordinal, native in enumerate(pe.functions):
        begin, end = native['beginRva'], native['endRva']
        code = pe.read(begin, end - begin)
        calls, refs, hashes = [], [], []
        normalized = bytearray(code)
        cursor = begin
        for ins in engine.disasm(code, begin):
            cursor = ins.address + ins.size
            for operand in ins.operands:
                if operand.type == X86_OP_MEM and operand.mem.base == X86_REG_RIP:
                    target = cursor + operand.mem.disp
                    mask_rip_displacement(normalized, ins, begin, operand.mem.disp)
                    if target in metadata_slots:
                        refs.append(dict(instructionRva=ins.address, targetRva=target,
                                         typeIndex=metadata_slots[target], kind='metadata-pointer-slot',
                                         mnemonic=ins.mnemonic))
                    position = bisect.bisect_right(metadata_starts, target) - 1
                    if position >= 0 and target < metadata_starts[position] + 0x90:
                        descriptor = metadata_starts[position]
                        refs.append(dict(instructionRva=ins.address, targetRva=target,
                                         typeIndex=metadata[descriptor], metadataOffset=target - descriptor,
                                         mnemonic=ins.mnemonic))
                elif operand.type == X86_OP_IMM:
                    if ins.group(cs.CS_GRP_JUMP) or ins.group(cs.CS_GRP_CALL):
                        offset = ins.address - begin + ins.imm_offset
                        normalized[offset:offset + ins.imm_size] = b'\0' * ins.imm_size
                    if ins.id in (cs.x86.X86_INS_CALL, cs.x86.X86_INS_JMP):
                        calls.append(dict(instructionRva=ins.address, targetRva=operand.imm,
                                          kind='call' if ins.id == cs.x86.X86_INS_CALL else 'jump'))
                    elif (operand.imm & 0xffffffff) in hash_types and -(1 << 31) <= operand.imm <= 0xffffffff:
                        value = operand.imm & 0xffffffff
                        hashes.append(dict(instructionRva=ins.address, value=value, candidates=hash_types[value],
                                           status='immediate-equality-only'))
        decoded_bytes += cursor - begin
        if cursor != end:
            undecoded.append(dict(beginRva=cursor, endRva=end))
        functions.append(dict(native, id=f'rva.{begin:08x}', codeSha256=hashlib.sha256(code).hexdigest(),
                              relocatedCodeSha256=hashlib.sha256(normalized).hexdigest() if cursor == end else None,
                              decodedBytes=cursor - begin, directBranches=calls, metadataReferences=refs,
                              hashCandidates=hashes, semanticStatus='unverified', callable=False))
        if ordinal and ordinal % 10000 == 0:
            progress(f'disassembled {ordinal}/{len(pe.functions)} compiler ranges')
    pointers = []
    for section in pe.sections:
        if section['flags'] & 0x20000000:
            continue
        prefix = (-section['rva']) % 8
        data = pe.read(section['rva'], section['rawSize'])
        values = np.frombuffer(data, dtype='<u8', offset=prefix, count=(len(data) - prefix) // 8)
        offsets = np.flatnonzero((values >= pe.base) & (values < pe.base + pe.image_size))
        for slot in offsets.tolist():
            rva = int(values[slot]) - pe.base
            if pe.executable(rva):
                idx = pe.containing_function(rva)
                pointers.append(dict(slotRva=section['rva'] + prefix + slot * 8, targetRva=rva,
                                     containingFunctionRva=pe.functions[idx]['beginRva'] if idx is not None else None))
    covered = sum(f['endRva'] - f['beginRva'] for f in pe.functions)
    groups = group_functions(pe, functions)
    return dict(functions=functions, functionGroups=groups, codePointers=pointers, undecodedRanges=undecoded,
                unknownUnwindVersions=pe.unknown_unwind_versions,
                metadataPointerSlots=[dict(slotRva=rva, typeIndex=index) for rva, index in sorted(metadata_slots.items())],
                coverage=dict(compilerRanges=len(functions), rangeBytes=covered, decodedBytes=decoded_bytes,
                              chainedFunctionGroups=len(groups),
                              functionsWithMetadataReferences=sum(bool(f['metadataReferences']) for f in functions),
                              functionsWithHashCandidates=sum(bool(f['hashCandidates']) for f in functions),
                              semanticBindingsProven=0, allEngineFunctionsEnumerated=False),
                limitations=['PE exception ranges may be fragments and omit leaf/inlined functions',
                             'metadata references and matching hash immediates do not prove function arguments or semantics',
                             'indirect calls and runtime-generated code require additional analysis'])
