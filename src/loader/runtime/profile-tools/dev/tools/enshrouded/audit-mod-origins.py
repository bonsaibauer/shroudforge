#!/usr/bin/env python3
"""Verify bundled mod origins against an exact EXE, full call inventory and KFC export.

Read-only. Does not promote unproved server operations or execute game code.
"""
import argparse
from collections import defaultdict
import json
from pathlib import Path
import re
import struct

from capstone import Cs, CS_ARCH_X86, CS_MODE_64
from capstone.x86 import X86_REG_RBP, X86_REG_RSP, X86_OP_MEM, X86_OP_REG
from discovery.pe import PE
from discovery.systems import discover as discover_systems


def validate_payload(spec):
    payload = bytes(spec['payload'])
    if spec['kind'] == 'bytes':
        assert len(payload) == spec['overwriteBytes']
    else:
        offset = spec['returnRel32Offset']
        assert payload[offset - 1] == 0xe9 and offset + 4 <= len(payload)
    for ref in spec.get('inlineReferences', []):
        offset, next_, data = (ref[k] for k in ['displacementOffset', 'nextInstructionOffset', 'dataOffset'])
        assert offset + 4 == next_ and data >= spec['returnRel32Offset'] + 4
        assert 0 < ref['dataSize'] <= len(payload) - data
        assert next_ + struct.unpack_from('<i', payload, offset)[0] == data, 'RIP-relative load does not address its inline constant'
        assert abs(struct.unpack_from('<f', payload, data)[0] - (-1.57)) < 1e-6
    if spec['modifier']['id'] == 'preserve_health_on_fall':
        assert payload[:4] == b'\x90' * 4, 'Health preservation must suppress the store, not add the computed value'
        assert payload[4:7] == bytes.fromhex('48 8b cb')
    return True


def audit(pe, profile, functions, resources):
    assert profile['image']['sha256'] == pe.sha256, 'profile belongs to another image'
    assert functions['image']['sha256'] == pe.sha256, 'call inventory belongs to another image'
    assert Path(resources['executable']).resolve().samefile(pe.path), 'KFC export belongs to another executable'
    descriptors = discover_systems(pe, functions)
    names = defaultdict(set)
    for d in descriptors:
        names[d['functionRva']].add(d['name'])
    edges = defaultdict(set)
    for function in functions['functions']:
        for branch in function['directBranches']:
            if branch['kind'] == 'call': edges[function['primaryBeginRva']].add(branch['targetRva'])
    attributes = {}
    queries = []
    for resource in resources['resources']:
        if resource['qualifiedType'] == 'keen::AttributeContainerResource':
            for root in resource['value']['attributes']:
                assert len(root['ids']) == len(root['debugNames']) == len(root['structure'])
                for index, (id_, name) in enumerate(zip(root['ids'], root['debugNames'])):
                    assert id_['value'] not in attributes
                    attributes[id_['value']] = dict(name=name, index=index, root_hash=root['ids'][0]['value'])
        if resource['qualifiedType'] == 'keen::GameKnowledgeQueryResourceDb':
            queries.extend(resource['value']['queries'])
    engine = Cs(CS_ARCH_X86, CS_MODE_64)
    engine.detail = True
    mods = []
    for alias, spec in profile['runtimePatches'].items():
        validate_payload(spec)
        modifier = spec['modifier']
        site = spec['function']['beginRva'] + spec['function']['targetOffset']
        owner = next(f for f in pe.functions if f['beginRva'] <= site < f['endRva'])
        assert owner['primaryBeginRva'] == modifier['function_rva']
        signature = bytes(0 if x in ('?', '??') else int(x, 16) for x in spec['signature'].split())
        original = pe.read(site, len(signature))
        assert all(x in ('?', '??') or original[i] == int(x, 16) for i, x in enumerate(spec['signature'].split()))
        if 'argument_index' in modifier:
            rsp_delta, rbp_delta = 0, None
            for instruction in engine.disasm(pe.read(owner['primaryBeginRva'], 96), owner['primaryBeginRva']):
                if instruction.mnemonic == 'push': rsp_delta -= 8
                elif instruction.mnemonic == 'sub' and instruction.operands[0].reg == X86_REG_RSP:
                    rsp_delta -= instruction.operands[1].imm
                elif instruction.mnemonic == 'lea' and instruction.operands[0].reg == X86_REG_RBP and instruction.operands[1].mem.base == X86_REG_RSP:
                    rbp_delta = rsp_delta + instruction.operands[1].mem.disp
                    break
            load = next(engine.disasm(original, site))
            assert rbp_delta is not None and load.operands[1].type == X86_OP_MEM and load.operands[1].mem.base == X86_REG_RBP
            offset = rbp_delta + load.operands[1].mem.disp
            # Return address + 32-byte home space => fifth argument at +0x28.
            assert offset >= 0x28 and (offset - 0x28) % 8 == 0
            assert 5 + (offset - 0x28) // 8 == modifier['argument_index']
        if 'engine_name' in modifier:
            assert modifier['engine_name'] in names[owner['primaryBeginRva']]
        for requirement in modifier.get('attribute_requirements', []):
            assert attributes[requirement['hash']] == {k: requirement[k] for k in ['name', 'index', 'root_hash']}
            # The selected function must actually reference the stated ID. It is
            # insufficient for the ID merely to exist in a separate resource.
            assert any(struct.pack('<I', requirement['hash']) in pe.read(f['beginRva'], f['endRva'] - f['beginRva'])
                       for f in pe.functions if f['primaryBeginRva'] == owner['primaryBeginRva'])
        for caller in modifier['callers']:
            path = caller['path_rvas']
            assert caller['name'] in names[path[0]] and path[-1] == owner['primaryBeginRva']
            assert all(b in edges[a] for a, b in zip(path, path[1:]))
        mods.append(dict(mod='sf-' + alias.rsplit('.', 1)[1].replace('_', '-'),
                         modifier=modifier, original_instructions=[f'{i.mnemonic} {i.op_str}' for i in engine.disasm(original[:spec['overwriteBytes']], site)],
                         code_and_origin_verified=True, gameplay_executed_by_audit=False))
    knowledge = set()
    for query in queries:
        if query['name'] == 'Unlock_Flame_Altar_PK':
            for action in query['actions']:
                if action['name'] == 'NPC_Flame_Hint01':
                    knowledge.add(action['query']['knowledgeOrQueryId']['value'])
    assert len(knowledge) == 1, 'blueprint source query/action is missing or ambiguous'
    mods.append(dict(mod='sf-unlock-blueprints', source_type='keen::GameKnowledgeQueryResourceDb',
                     query='Unlock_Flame_Altar_PK', action='NPC_Flame_Hint01', knowledge_id=next(iter(knowledge)),
                     destination='keen::RecipeRegistryResource.recipes[].knowledgeRequirement'))
    mods.append(dict(mod='sf-production-time', source_type='keen::RecipeRegistryResource',
                     destination='recipes[].craftingDuration.value', unit='nanoseconds',
                     selection='positive duration only', native_patch=False,
                     note='Absolute duration through the existing asset API; identical settings required in both installations.'))
    world = []
    for name, operation in profile['worldOperations'].items():
        actor_context = operation['abi'] == 'actor-world-context'
        address = operation.get('functionRva', operation.get('globalRva', operation.get('guardRva', 0) if actor_context else 0))
        guards = bytes(operation.get('guardBytes', []))
        try:
            guard_matches = pe.read(operation.get('guardRva', 0), len(guards)) == guards if guards else None
        except ValueError:
            guard_matches = False
        if operation.get('validated', True):
            assert 0 < address < pe.image_size, f'{name}: address outside target image'
            assert guard_matches is not False, f'{name}: code guard mismatch'
        world.append(dict(id=name, address_in_image=0 < address < pe.image_size, guard_matches=guard_matches,
                          native_call_executed=False, context_source='actor-frame' if actor_context else operation['context'],
                          abi=operation['abi'], context=operation['context']))
    # Hooks are target-specific executable signatures, never copied client
    # addresses. Prove uniqueness and complete overwritten instructions.
    for name, hook in profile['hooks'].items():
        if not name.startswith('world_'): continue
        tokens = hook['signature'].split()
        pattern = re.compile(b''.join(b'.' if t in ('?', '??') else re.escape(bytes([int(t,16)])) for t in tokens), re.DOTALL)
        matches = []
        for section in pe.sections:
            if section['flags'] & 0x20000000:
                data = pe.data[section['raw']:section['raw']+section['rawSize']]
                matches += [section['rva']+m.start() for m in pattern.finditer(data)]
        assert len(matches) == 1, f'{name}: expected one executable signature, got {matches}'
        assert pe.read(matches[0], len(hook['original'])) == bytes(hook['original'])
        instructions = list(Cs(CS_ARCH_X86,CS_MODE_64).disasm(bytes(hook['original']), matches[0]))
        assert sum(i.size for i in instructions) == len(hook['original']), f'{name}: partial instruction overwrite'
    mods.append(dict(mod='world-editor', manifest_target='client', operations=world,
                     note='Client cursor/UI; direct world calls and optional ordinary ClientPlayerInput queue. Dispatch, replication and persistence are distinct; this static audit does not execute gameplay.'))
    return dict(image=pe.identity(), attribute_ids=len(attributes), engine_descriptors=len(descriptors),
                mods=mods, complete_engine_api=False)


def audit_packages(result, root, target):
    """Join actual Lua entrypoints to verified native origins, not mod titles."""
    package_ids = {json.loads(p.read_text(encoding='utf-8-sig'))['id'] for p in root.glob('*/mod.json')}
    assert package_ids == {row['mod'] for row in result['mods']}, 'audit must cover every bundled mod'
    for row in result['mods']:
        directory = root / row['mod']
        manifest = json.loads((directory / 'mod.json').read_text(encoding='utf-8-sig'))
        extended = json.loads((directory / 'extended.mod.json').read_text(encoding='utf-8-sig'))
        source = (directory / 'src/mod.lua').read_text(encoding='utf-8-sig')
        row.update(targets=extended['targets'], runs_on_target=target in extended['targets'],
                   scope='this-process', multiplayer_gameplay_verified=False)
        if 'modifier' in row:
            modifier_id = re.search(r'local modifier_id = "([^"]+)"', source)
            assert modifier_id and modifier_id[1] == row['modifier']['id']
            assert 'runtime.functions.bind_modifier(modifier_id)' in source
            assert 'runtime' in manifest['capabilities']
            row['phase'] = 'ingame'
        elif row['mod'] == 'sf-unlock-blueprints':
            assert all(name in source for name in (row['query'], row['action'], 'recipe.knowledgeRequirement'))
            assert 'patch' in manifest['capabilities']
            row['phase'] = 'pregame-assets'
        elif row['mod'] == 'sf-production-time':
            assert all(name in source for name in (row['source_type'], 'recipe.craftingDuration',
                                                  'time.value > 0', 'seconds * 1000000000'))
            assert manifest['capabilities'] == ['patch']
            assert extended['targets'] == ['client', 'server']
            row['phase'] = 'pregame-assets'
        else:
            assert extended['targets'] == ['client']
            assert 'executionMode' in extended['settings'] and 'paste_through_game' in source
            row['phase'] = 'ingame'
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('executable', type=Path)
    p.add_argument('--profile', type=Path, required=True)
    p.add_argument('--functions', type=Path, required=True)
    p.add_argument('--resources', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--mods', type=Path, default=Path(__file__).resolve().parents[7] / 'mods')
    args = p.parse_args()
    read = lambda path: json.loads(path.read_text(encoding='utf-8-sig'))
    result = audit(PE(args.executable), read(args.profile), read(args.functions), read(args.resources))
    audit_packages(result, args.mods, 'server' if 'server' in args.executable.stem.lower() else 'client')
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(f"Verified {len(result['mods'])} mod origins, {result['attribute_ids']} attribute IDs, {result['engine_descriptors']} engine descriptors")


if __name__ == '__main__':
    main()
