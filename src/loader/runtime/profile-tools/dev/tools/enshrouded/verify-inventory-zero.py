"""Execute original inventory decrement helpers in Unicorn, never in the game.

Tests the zero quantity passed by zero_resource_argument against normal quantities.
Engine lookups/record allocation are isolated stubs; this is NOT a live factory test.
"""
import argparse
import json
import struct
from pathlib import Path

from capstone import Cs, CS_ARCH_X86, CS_MODE_64
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_RSP, UC_X86_REG_RIP, UC_X86_REG_RAX, UC_X86_REG_RCX, UC_X86_REG_RDX, UC_X86_REG_R8, UC_X86_REG_R9
from discovery.pe import PE


BUILDS = {
    '001c1b40ed091d8c1aee583adde3800d7c858ae2c7f4dff54fca2938b2be1637': (0x164170, 0x1642f0),
    'af2f5a1227911d8aa06b3908d6bd0211838211cae14ea91099cb57d0df990781': (0x3825b0, 0x382730),
}


def run(pe, root, explicit_slot, amount):
    function = next(f for f in pe.functions if f['beginRva'] == root)
    code = pe.read(root, function['endRva'] - root)
    instructions = list(Cs(CS_ARCH_X86, CS_MODE_64).disasm(code, root))
    calls = [i for i in instructions if i.mnemonic == 'call']
    if len(calls) != 4 or any(not i.op_str.startswith('0x') for i in calls):
        raise ValueError('unexpected helper call contract')
    # Both exact images have: slot key, writable-slot check, stack decode, journal allocation.
    call_index = {pe.base + i.address: index for index, i in enumerate(calls)}
    uc = Uc(UC_ARCH_X86, UC_MODE_64)
    page = (pe.base + root) & ~0xfff
    uc.mem_map(page, 0x2000)
    uc.mem_write(pe.base + root, code)
    data, stack, stop = 0x20000000, 0x30000000, 0x40000000
    uc.mem_map(data, 0x10000)
    uc.mem_map(stack, 0x10000)
    uc.mem_map(stop, 0x1000)
    rsp = stack + 0x8008
    result, context, view, slots, journal = (data + x for x in (0, 0x100, 0x1000, 0x2000, 0x3000))
    item, initial = 455684957, 20
    uc.mem_write(view, struct.pack('<QQ', slots, 0))
    uc.mem_write(slots, struct.pack('<III', item, initial, 0) + bytes(84))
    uc.mem_write(rsp, struct.pack('<Q', stop))
    args = (0, item, amount) if explicit_slot else (item, amount)
    for index, value in enumerate(args):
        uc.mem_write(rsp + 40 + index * 8, struct.pack('<Q', value))
    for register, value in [(UC_X86_REG_RSP, rsp), (UC_X86_REG_RCX, result),
                            (UC_X86_REG_RDX, context), (UC_X86_REG_R8, view), (UC_X86_REG_R9, 123)]:
        uc.reg_write(register, value)

    def hook(machine, address, size, unused):
        if address not in call_index:
            return
        kind = call_index[address]
        if kind == 0:
            machine.reg_write(UC_X86_REG_RAX, 123)
        elif kind == 1:
            machine.reg_write(UC_X86_REG_RAX, 1)
        elif kind == 2:
            source, target = machine.reg_read(UC_X86_REG_RDX), machine.reg_read(UC_X86_REG_RCX)
            machine.mem_write(target, bytes(machine.mem_read(source, 12)))
            machine.reg_write(UC_X86_REG_RAX, target)
        else:
            machine.reg_write(UC_X86_REG_RAX, journal)
        machine.reg_write(UC_X86_REG_RIP, address + size)

    uc.hook_add(UC_HOOK_CODE, hook)
    uc.emu_start(pe.base + root, stop, count=5000)
    if uc.reg_read(UC_X86_REG_RIP) != stop:
        raise AssertionError('helper did not return within instruction budget')
    remaining = struct.unpack('<I', uc.mem_read(slots + 4, 4))[0]
    recorded = struct.unpack('<H', uc.mem_read(journal + 0x24, 2))[0]
    error = uc.mem_read(result, 1)[0]
    assert remaining == max(0, initial - amount), (hex(root), amount, remaining)
    assert recorded == min(initial, amount), (hex(root), amount, recorded)
    assert error == 0, (hex(root), amount, error)
    return dict(functionRva=root, explicitSlot=explicit_slot, requestedQuantity=amount,
                before=initial, after=remaining, journalQuantity=recorded)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    pe = PE(args.exe)
    roots = BUILDS[pe.sha256]
    cases = [run(pe, root, index == 0, amount)
             for index, root in enumerate(roots) for amount in (0, 1, 10, 20)]
    report = dict(image=pe.identity(), cases=cases, engineDependenciesStubbed=True,
                  liveFactoryConsumptionTested=False)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(f'{len(cases)} original-code cases passed; zero quantity preserves the source stack')


if __name__ == '__main__':
    main()
