"""Verify original Place/Destroy event ABIs in an isolated x64 emulator.

No game process is opened. Queue allocation and timestamp helpers are stubbed;
the actual event-writing instructions run from the SHA-identified executable.
This proves argument-to-event-field mapping, NOT replication or persistence.
"""
import argparse
import json
import struct
from pathlib import Path

from discovery.pe import PE
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE, UC_PROT_READ, UC_PROT_EXEC, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_RAX, UC_X86_REG_RCX, UC_X86_REG_RDX,
                              UC_X86_REG_R8, UC_X86_REG_R9, UC_X86_REG_RSP, UC_X86_REG_RIP)

BUILDS = {
    'af2f5a1227911d8aa06b3908d6bd0211838211cae14ea91099cb57d0df990781':
        dict(place=0x3ebb70, destroy=0x3ebcb0, queue_id=0x8d6690,
             queue=0x8d8040, allocate=0x878b90, timestamp=0x8d7aa0),
    '001c1b40ed091d8c1aee583adde3800d7c858ae2c7f4dff54fca2938b2be1637':
        dict(place=0x1c71c0, destroy=0x1c7300, queue_id=0x5d8350,
             queue=0x5d9e70, allocate=0x57a1b0, timestamp=0x5d98d0),
}
SCRATCH, STACK, STOP = 0x40000000, 0x50000000, 0x60000000


def verify(pe, types, kind, fourth, fifth):
    roots = BUILDS[pe.sha256]
    u = Uc(UC_ARCH_X86, UC_MODE_64)
    u.mem_map(pe.base, (pe.image_size + 4095) & ~4095, UC_PROT_READ | UC_PROT_EXEC)
    u.mem_write(pe.base, pe.data[:pe.headers_size])
    for s in pe.sections:
        u.mem_write(pe.base + s['rva'], pe.data[s['raw']:s['raw'] + s['rawSize']])
    u.mem_map(SCRATCH, 0x10000, UC_PROT_ALL)
    u.mem_map(STACK, 0x10000, UC_PROT_ALL)
    u.mem_map(STOP, 4096, UC_PROT_READ | UC_PROT_EXEC)
    event, transform, bounds = SCRATCH + 0x1000, SCRATCH + 0x2000, SCRATCH + 0x3000
    stamp = SCRATCH + 0x4000
    u.mem_write(SCRATCH, struct.pack('<Q', SCRATCH + 0x5000))
    for offset in [0xb0, 0xc0]:
        u.mem_write(SCRATCH + offset, struct.pack('<Q', SCRATCH + 0x6000))
    u.mem_write(SCRATCH + 0x134, struct.pack('<I', 0x12345678))
    u.mem_write(stamp, struct.pack('<Q', 0x1122334455667788))
    u.mem_write(transform, struct.pack('<3q4f4f', 1 << 32, 2 << 32, 3 << 32,
                                     0, 0, 0, 1, 1, 1, 1, 0))
    u.mem_write(bounds, struct.pack('<8f', -1, -2, -3, 0, 1, 2, 3, 0))
    rsp = STACK + 0xff08
    u.mem_write(rsp, struct.pack('<Q', STOP))
    u.mem_write(rsp + 0x28, struct.pack('<Q', fifth))
    for reg, value in [(UC_X86_REG_RCX, SCRATCH), (UC_X86_REG_RDX, transform),
                       (UC_X86_REG_R8, bounds), (UC_X86_REG_R9, fourth), (UC_X86_REG_RSP, rsp)]:
        u.reg_write(reg, value)
    ranges = [(pe.base + f['beginRva'], pe.base + f['endRva']) for f in pe.functions
              if f['primaryBeginRva'] == roots[kind]]
    allocation = []
    helpers = {roots['queue_id']: 1, roots['queue']: SCRATCH + 0x7000,
               roots['allocate']: event, roots['timestamp']: stamp}

    def on_code(uc, address, size, _):
        rva = address - pe.base
        if rva in helpers:
            if rva == roots['allocate']:
                allocation.append((uc.reg_read(UC_X86_REG_RDX), uc.reg_read(UC_X86_REG_R8)))
            sp = uc.reg_read(UC_X86_REG_RSP)
            ret = struct.unpack('<Q', uc.mem_read(sp, 8))[0]
            uc.reg_write(UC_X86_REG_RAX, helpers[rva])
            uc.reg_write(UC_X86_REG_RSP, sp + 8)
            uc.reg_write(UC_X86_REG_RIP, ret)
        elif not any(start <= address < end for start, end in ranges):
            raise ValueError(f'unexpected execution at {address:#x}')

    u.hook_add(UC_HOOK_CODE, on_code)
    u.emu_start(pe.base + roots[kind], STOP, timeout=100000, count=10000)
    if u.reg_read(UC_X86_REG_RIP) != STOP or len(allocation) != 1:
        raise ValueError('execution incomplete or unexpected allocation count')
    hash_value, size = allocation[0]
    matches = [t for t in types if t['qualifiedHash'] == hash_value]
    if len(matches) != 1 or matches[0]['size'] != size:
        raise ValueError('ambiguous event type or size mismatch')
    ty = matches[0]
    raw = bytes(u.mem_read(event, size))
    scalar = lambda name: struct.unpack_from('<I', raw, ty['structFields'][name]['dataOffset'])[0]
    if scalar('material') != fourth or scalar('ownerId') != 0x12345678:
        raise ValueError('material/owner ABI mismatch')
    if kind == 'place' and scalar('trackingItemId') != fifth:
        raise ValueError('trackingItemId ABI mismatch')
    if struct.unpack_from('<Q', raw)[0] != 0x1122334455667788:
        raise ValueError('timestamp mismatch')
    if struct.unpack_from('<3f', raw, ty['structFields']['position']['dataOffset']) != (1, 2, 3):
        raise ValueError('position ABI mismatch')
    return dict(operation=kind, functionRva=roots[kind], event=ty['qualifiedName'],
                qualifiedHash=hash_value, size=size, fourthArgument='material: keen::MaterialFeedbackId',
                fifthArgument='trackingItemId: keen::ItemId' if kind == 'place' else None,
                fieldValues={n: scalar(n) for n in ['material', 'ownerId'] + (['trackingItemId'] if kind == 'place' else [])})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', required=True)
    parser.add_argument('--reflection', required=True)
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    pe = PE(args.exe)
    reflection = json.loads(Path(args.reflection).read_text(encoding='utf-8'))
    if reflection['image']['sha256'] != pe.sha256:
        raise ValueError('reflection belongs to a different executable')
    results = [verify(pe, reflection['types'], kind, a, b)
               for kind in ['place', 'destroy']
               for a, b in [(0x1234abcd, 0x5678ef01), (0, 0xffffffff)]]
    output = dict(executableSha256=pe.sha256, cases=results,
                  limitations=['Queue allocation and timestamp helpers are stubbed.',
                               'No live world mutation, network or persistence test.'])
    Path(args.out).write_text(json.dumps(output, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(executableSha256=pe.sha256, passed=len(results))))


if __name__ == '__main__':
    main()
