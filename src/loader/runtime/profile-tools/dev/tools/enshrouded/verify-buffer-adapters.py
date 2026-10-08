#!/usr/bin/env python3
"""Differentially verify proven copy adapters on private buffers, never on game memory."""
import argparse
import ctypes as c
import json
import random
from pathlib import Path

from discovery.pe import PE


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--registry', type=Path, required=True)
    parser.add_argument('--adapters', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    pe = PE(args.executable)
    registry = json.loads(args.registry.read_text())
    adapters = json.loads(args.adapters.read_text())['adapters']
    evidence = {cb['function_rva']: cb['code_hex'] for row in registry['entries'] for cb in row['callbacks']}
    import capstone as cs
    engine = cs.Cs(cs.CS_ARCH_X86, cs.CS_MODE_64)
    kernel = c.WinDLL('kernel32', use_last_error=True)
    kernel.VirtualAlloc.argtypes = [c.c_void_p, c.c_size_t, c.c_uint32, c.c_uint32]
    kernel.VirtualAlloc.restype = c.c_void_p
    kernel.VirtualProtect.argtypes = [c.c_void_p, c.c_size_t, c.c_uint32, c.POINTER(c.c_uint32)]
    kernel.VirtualFree.argtypes = [c.c_void_p, c.c_size_t, c.c_uint32]
    kernel.GetCurrentProcess.restype = c.c_void_p
    kernel.FlushInstructionCache.argtypes = [c.c_void_p, c.c_void_p, c.c_size_t]
    results = []
    for adapter in adapters:
        plan, rva = adapter['buffer_transform'], adapter['rva']
        code = bytes.fromhex(evidence[rva])[:plan['code_length']]
        assert code == pe.read(rva, len(code)), 'live code differs from selected executable'
        instructions = list(engine.disasm(code, rva))
        assert instructions and instructions[-1].mnemonic == 'ret'
        assert sum(ins.size for ins in instructions) == len(code)
        assert all(ins.mnemonic in ('mov', 'movzx', 'ret') for ins in instructions)
        # Execute only the straight-line bodies already accepted by the Rust proof
        # checker and separately decoded above. No relocation or game context exists.
        address = kernel.VirtualAlloc(None, len(code), 0x3000, 0x04)
        if not address:
            raise c.WinError(c.get_last_error())
        try:
            c.memmove(address, code, len(code))
            previous = c.c_uint32()
            if not kernel.VirtualProtect(address, len(code), 0x20, c.byref(previous)):
                raise c.WinError(c.get_last_error())
            if not kernel.FlushInstructionCache(kernel.GetCurrentProcess(), address, len(code)):
                raise c.WinError(c.get_last_error())
            native = c.CFUNCTYPE(c.c_uint64, c.POINTER(c.c_void_p), c.POINTER(c.c_void_p))(address)
            randomizer = random.Random(rva)
            for trial in range(256):
                source = randomizer.randbytes(plan['source_minimum'] + 32)
                destination = randomizer.randbytes(plan['destination_minimum'] + 32)
                expected = bytearray(destination)
                for copy in plan['copies']:
                    si, di, size = copy['source_offset'], copy['destination_offset'], copy['size']
                    expected[16 + di:16 + di + size] = source[16 + si:16 + si + size]
                source_buffer = c.create_string_buffer(source, len(source))
                destination_buffer = c.create_string_buffer(destination, len(destination))
                source_slot = c.c_void_p(c.addressof(source_buffer) + 16)
                destination_slot = c.c_void_p(c.addressof(destination_buffer) + 16)
                result = native(c.byref(destination_slot), c.byref(source_slot))
                assert destination_buffer.raw == expected, (adapter['name'], trial, 'copy mismatch/guard overwritten')
                assert source_buffer.raw == source, 'source mutated'
                if plan['native_constant_result'] is not None:
                    assert result == plan['native_constant_result']
            results.append(dict(name=adapter['name'], rva=rva, trials=256,
                                nativeBodyBytes=len(code), originalCodeMatches=True,
                                bufferEffectsMatch=True, gameplayEffectsVerified=False))
        finally:
            kernel.VirtualFree(address, 0, 0x8000)
    report = dict(schemaVersion=1, image=pe.identity(), execution='isolated-copy-on-owned-buffers',
                  gameMemoryWritten=False, results=results)
    args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(f'{len(results)} original callback bodies matched the adapters in {len(results) * 256} differential trials')


if __name__ == '__main__':
    main()
