#!/usr/bin/env python3
"""Read-only proof of the exact client-player-input-v1 layout and hook contract.

This checks original EXE bytes and extracted reflection, not network delivery.
A new build must supply new evidence before the native adapter can be enabled.
"""
import argparse
import json
from pathlib import Path
from capstone import Cs, CS_ARCH_X86, CS_MODE_64
from discovery.pe import PE


def verify(pe, profile, reflection):
    assert profile['image']['sha256'] == reflection['image']['sha256'] == pe.sha256
    assert profile['target'] == 'enshrouded.exe'
    types = {t['qualifiedName']: t for t in reflection['types']}
    expected = {
        'keen::ecs::ClientPlayerInput': (1392, {'data': 0}),
        'keen::ecs::ClientPlayerInputData': (808, {'buildingStockCycleAction': 164,
            'createBuildingItemAction': 432, 'cursorInput': 624, 'digitalInput': 792}),
        'keen::ecs::CreateBuildingItemAction': (12, {'versionData': 0, 'selectedIndex': 4, 'itemId': 8}),
        'keen::ecs::BuildingStockCycleAction': (20, {'versionData': 0, 'terrainMaterialItemId': 4,
            'blueprintMaterialDefaultItemId': 8, 'blueprintMaterialRoofItemId': 12, 'overgrowthMaterialItemId': 16}),
        'keen::ecs::ClientCursorInput': (160, {'primaryTransform': 0, 'secondaryTransform': 56,
            'primaryClientFlags': 112, 'secondaryClientFlags': 113}),
        'keen::WorldTransform': (56, {'position': 0, 'orientation': 24, 'scale': 40}),
        'keen::ecs::PlayerInput': (1320, {'fromClient': 8}),
    }
    for name, (size, fields) in expected.items():
        ty = types[name]
        assert ty['size'] == size, name
        for field, offset in fields.items():
            assert ty['structFields'][field]['dataOffset'] == offset, (name, field)
    enum = types['keen::ecs::PlayerInputType']['enumFields']
    for name, value in {'MainhandAction': 0, 'MainhandAction_Tap': 1, 'MainhandAction_Hold': 2,
                        'MainhandAction_Release': 3, 'SecondaryBuildingAction': 4, 'BuildingUndo': 37}.items():
        assert enum[name]['value'] == value
    operations = profile['worldOperations']
    for name in ('runtime.world.building.input', 'runtime.world.building.version'):
        op = operations[name]
        assert pe.read(op['guardRva'], len(op['guardBytes'])) == bytes(op['guardBytes'])
    version = operations['runtime.world.building.version']['functionRva']
    assert pe.read(version, 7) == bytes.fromhex('48 8b 01 8b 40 6c c3')
    # This exact build has both paths restoring R13 before the existing cursor
    # hook. The game calls its version helper with that same execution view.
    engine = Cs(CS_ARCH_X86, CS_MODE_64)
    code = {i.address: f'{i.mnemonic} {i.op_str}' for i in engine.disasm(pe.read(0x24a9fe, 0x40), 0x24a9fe)}
    assert code[0x24a9fe] == code[0x24aa16] == 'mov r13, qword ptr [rbp + 0x2500]'
    assert code[0x24aa05] == 'mov rcx, r13'
    assert code[0x24aa08] == f'call {hex(version)}'
    assert code[0x24aa1d] == 'mov edx, dword ptr [r14 + 0x2f8]'
    owner = next(f for f in pe.functions if f['beginRva'] <= 0x24aa1d < f['endRva'])
    assert owner['primaryBeginRva'] == operations['runtime.world.building.input']['functionRva']
    return {'image': pe.identity(), 'model': 'client-player-input-v1', 'layout_types_checked': len(expected),
            'cursor_hook_rva': 0x24aa1d, 'input_register': 'r14', 'execution_view_register': 'r13',
            'version_helper_rva': version, 'network_delivery_tested': False, 'world_effect_tested': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--reflection', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    read = lambda path: json.loads(path.read_text(encoding='utf-8-sig'))
    report = verify(PE(args.executable), read(args.profile), read(args.reflection))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('Verified 7 original input layouts, 6 action bits, cursor registers and original version helper; no game process modified')
