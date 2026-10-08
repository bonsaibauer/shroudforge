"""Generate differential-test vectors by emulating the ORIGINAL game interpreters.

The PE is data in Unicorn, never loaded/executed by Windows. Only private scratch
and stack pages are writable; imports, invalid accesses and timeouts fail closed.
No game process is opened. RVAs below are independently disassembled build facts.
"""
import argparse
import json
import random
import struct
from pathlib import Path

from discovery.pe import PE
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_PROT_READ, UC_PROT_EXEC, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_RCX, UC_X86_REG_RDX, UC_X86_REG_R8,
                              UC_X86_REG_R9, UC_X86_REG_RSP, UC_X86_REG_RIP, UC_X86_REG_MXCSR)

BUILDS = {
    'af2f5a1227911d8aa06b3908d6bd0211838211cae14ea91099cb57d0df990781':
        {'sint32': 0x1e1b00, 'uint32': 0x214090, 'float32': 0x1f3040},
    '001c1b40ed091d8c1aee583adde3800d7c858ae2c7f4dff54fca2938b2be1637':
        {'sint32': 0x2c310, 'uint32': 0x4e1c0, 'float32': 0x3d4f0},
}
SCALARS = {2556031774: 'sint32', 848563180: 'uint32', 3902764048: 'float32'}


class Oracle:
    SCRATCH = 0x40000000
    STACK = 0x50000000
    STOP = 0x60000000

    def __init__(self, pe):
        self.pe = pe
        self.roots = BUILDS[pe.sha256]
        self.uc = Uc(UC_ARCH_X86, UC_MODE_64)
        self.uc.mem_map(pe.base, (pe.image_size + 4095) & ~4095, UC_PROT_READ | UC_PROT_EXEC)
        self.uc.mem_write(pe.base, pe.data[:pe.headers_size])
        for s in pe.sections:
            self.uc.mem_write(pe.base + s['rva'], pe.data[s['raw']:s['raw'] + s['rawSize']])
        self.uc.mem_map(self.SCRATCH, 0x10000, UC_PROT_ALL)
        self.uc.mem_map(self.STACK, 0x10000, UC_PROT_ALL)
        self.uc.mem_map(self.STOP, 4096, UC_PROT_READ | UC_PROT_EXEC)

    def execute(self, kind, values, words):
        if len(values) > 1024 or len(words) > 1024:
            raise ValueError('oversized test vector')
        u = self.uc
        u.mem_write(self.SCRATCH, bytes(0x10000))
        u.mem_write(self.STACK, bytes(0x10000))
        u.mem_write(self.SCRATCH + 0x1000, struct.pack('<' + 'I' * len(values), *values))
        u.mem_write(self.SCRATCH + 0x3000, struct.pack('<' + 'I' * len(words), *words))
        u.mem_write(self.SCRATCH + 0x100, struct.pack('<QQ', self.SCRATCH + 0x1000, len(values)))
        u.mem_write(self.SCRATCH + 0x200, struct.pack('<QQ', self.SCRATCH + 0x3000, len(words)))
        rsp = self.STACK + 0xff08
        u.mem_write(rsp, struct.pack('<Q', self.STOP))
        for r, value in [(UC_X86_REG_RCX, self.SCRATCH), (UC_X86_REG_RDX, self.SCRATCH + 0x100),
                         (UC_X86_REG_R8, self.SCRATCH + 0x200), (UC_X86_REG_R9, 0),
                         (UC_X86_REG_RSP, rsp), (UC_X86_REG_MXCSR, 0x1f80)]:
            u.reg_write(r, value)
        u.emu_start(self.pe.base + self.roots[kind], self.STOP, timeout=100000, count=100000)
        if u.reg_read(UC_X86_REG_RIP) != self.STOP:
            raise ValueError('interpreter exceeded execution budget')
        status, result = struct.unpack('<B3xI', u.mem_read(self.SCRATCH, 8))
        after = list(struct.unpack('<' + 'I' * len(values), u.mem_read(self.SCRATCH + 0x1000, len(values) * 4)))
        return dict(status=status, result=result, after=after)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', required=True)
    parser.add_argument('--resources', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--samples', type=int, default=32)
    args = parser.parse_args()
    pe = PE(args.exe)
    oracle = Oracle(pe)
    resources = json.loads(Path(args.resources).read_text())
    roots = next(r['value']['attributes'] for r in resources['resources']
                 if r['qualifiedType'] == 'keen::AttributeContainerResource')
    rng = random.Random(20261009)
    vectors = []
    for root in roots:
        kind = SCALARS[root['type']['value']]
        count = len(root['ids'])
        for i, row in enumerate(root['structure']):
            words = row['calculation']
            if not words:
                continue
            for sample in range(args.samples):
                if kind == 'float32':
                    values = [struct.unpack('<I', struct.pack('<f', rng.uniform(-100, 100)))[0] for _ in range(count)]
                else:
                    values = [rng.randint(-10000 if kind == 'sint32' else 0, 10000) & 0xffffffff for _ in range(count)]
                if sample < 3:
                    values = [[0, 1, 0x7fffffff][sample]] * count if kind != 'float32' else [struct.unpack('<I', struct.pack('<f', float(sample)))[0]] * count
                native = oracle.execute(kind, values, words)
                vectors.append(dict(name=root['debugNames'][i], kind=kind, values=values, words=words, **native))
    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(dict(executableSha256=pe.sha256, interpreters=oracle.roots,
        environment='Unicorn x86_64; isolated read-only PE; no host native execution', vectors=vectors)))
    print(json.dumps(dict(executableSha256=pe.sha256, programs=len(vectors) // args.samples,
                          samples=len(vectors), nativeStatuses=sorted(set(v['status'] for v in vectors)))))


if __name__ == '__main__':
    main()
