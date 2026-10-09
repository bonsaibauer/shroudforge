#!/usr/bin/env python3
"""Verify the client_cursor -> VoxelWorld -> existing voxel reader data flow.

Reads the original EXE only. Does not attach, suspend, inject or call the game.
"""
import argparse
import json
from pathlib import Path
from capstone import Cs, CS_ARCH_X86, CS_MODE_64
from discovery.pe import PE


def verify(pe, profile):
    assert pe.sha256 == profile['image']['sha256']
    assert profile['target'] == 'enshrouded.exe'
    layout = profile['worldContexts']['clientCursorRead']
    assert layout['frameServiceViewOffset'] == 0x250
    assert layout['serviceWorldOffset'] == 8
    decoder = Cs(CS_ARCH_X86, CS_MODE_64)

    def instruction(rva, expected):
        item = next(decoder.disasm(pe.read(rva, 15), rva))
        assert f'{item.mnemonic} {item.op_str}' == expected, (hex(rva), item.op_str)

    # The service view stays in the cursor's own frame. The accessor returns
    # its value pointer, which becomes the world argument to the voxel reader.
    instruction(0x24984d, 'mov rax, qword ptr [rbp + 0x250]')
    instruction(0x249854, 'mov qword ptr [rbp + 0x4e8], rax')
    instruction(0x249f59, 'mov rcx, qword ptr [rbp + 0x4e8]')
    instruction(0x249f60, 'call 0x99e530')
    assert pe.read(0x99e530, 5) == bytes.fromhex('48 8b 41 08 c3')
    instruction(0x249f65, 'mov rcx, rax')
    instruction(0x249f72, 'call 0xe8b710')
    instruction(0xe8b74c, 'mov rbx, rcx')
    instruction(0xe8b786, 'mov r9d, 6')
    instruction(0xe8b792, 'mov r8, rbx')
    reader = profile['worldOperations']['runtime.world.voxel.read']
    instruction(0xe8b7e2, f"call {hex(reader['functionRva'])}")
    assert reader['mode'] == 6
    assert pe.read(reader['guardRva'], len(reader['guardBytes'])) == bytes(reader['guardBytes'])
    instruction(0x24aa1d, 'mov edx, dword ptr [r14 + 0x2f8]')
    # The additional callback argument preserves the existing stack contract.
    args = list(decoder.disasm(bytes.fromhex('49 8b d5 49 89 e8'), 0))
    assert [(i.mnemonic, i.op_str) for i in args] == [('mov', 'rdx, r13'), ('mov', 'r8, rbp')]
    return {'image': pe.identity(), 'source': 'client-cursor-frame',
            'frameServiceViewOffset': 0x250, 'serviceWorldOffset': 8,
            'readerRva': reader['functionRva'], 'writesAllowed': False,
            'liveReadVerified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    result = verify(PE(args.executable), json.loads(args.profile.read_text()))
    args.out.write_text(json.dumps(result, indent=2) + '\n')
    print('Client voxel context instruction/ABI proof passed; no game process opened')
