import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
from discovery.pe import PE
from discovery.reflection import extract, fnv
from discovery.functions import discover, mask_rip_displacement


def fixture():
    """Independent tiny PE with two type descriptors and duplicate registry handles."""
    data = bytearray(0x1200)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 0x80)
    data[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<HHIIIHH', data, 0x84, 0x8664, 1, 123, 0, 0, 240, 0)
    opt = 0x98
    struct.pack_into('<H', data, opt, 0x20b)
    struct.pack_into('<Q', data, opt + 24, 0x140000000)
    struct.pack_into('<II', data, opt + 56, 0x2000, 0x200)
    struct.pack_into('<8sIIIIIIHHI', data, opt + 240, b'.rdata', 0x1000, 0x1000,
                     0x1000, 0x200, 0, 0, 0, 0, 0x40000040)
    for name, text_rva, metadata, primitive, size in [
        (b'BlobString', 0x1001, 0x1100, 25, 16), (b'uint32', 0x1021, 0x1190, 6, 4)
    ]:
        raw = text_rva - 0xe00
        data[raw:raw + len(name)] = name
        struct.pack_into('<8Q', data, metadata - 0xe00,
                         0x140000000 + text_rva, len(name), 0x140000000 + text_rva, len(name),
                         0x140000000 + text_rva, len(name), 0, 0)
        struct.pack_into('<IHHIBB2xII', data, metadata - 0xe00 + 0x40,
                         size, 4, 4, 0, primitive, 0, fnv(name.decode()), 777)
    struct.pack_into('<QQ', data, 0x500, 0x140001100, 0x140001190)
    for raw in (0x540, 0x550):
        struct.pack_into('<QQ', data, raw, 0x140001300, 2)
    return data


class DiscoveryTest(unittest.TestCase):
    def load(self, data):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / 'game.exe'
        path.write_bytes(data)
        return PE(path)

    def test_extracts_registry_without_profile_and_preserves_collisions(self):
        result = extract(self.load(fixture()))
        self.assertEqual(result['headerRvas'], [0x1340, 0x1350])
        self.assertEqual(result['count'], 2)
        self.assertEqual(result['types'][1]['qualifiedName'], 'uint32')
        self.assertEqual(result['types'][1]['metadataRva'], 0x1190)
        self.assertEqual(result['hashCollisions']['internalHash'], {'777': [0, 1]})

    def test_malformed_type_reference_fails_instead_of_dropping_a_field(self):
        data = fixture()
        struct.pack_into('<Q', data, 0x300 + 0x38, 0x140001555)
        with self.assertRaisesRegex(ValueError, 'unresolved reflected type'):
            extract(self.load(data))

    def test_truncated_registry_cannot_be_reported_as_complete(self):
        data = fixture()
        for raw in (0x548, 0x558):
            struct.pack_into('<Q', data, raw, 100000)
        with self.assertRaisesRegex(ValueError, 'reflection registry'):
            extract(self.load(data))

    def test_file_bounds_and_preferred_base(self):
        pe = self.load(fixture())
        self.assertEqual(pe.rva(0x140001190), 0x1190)
        with self.assertRaises(ValueError):
            pe.read(0x1fff, 8)
        with self.assertRaises(ValueError):
            pe.rva(0x180001190)
        with self.assertRaises(ValueError):
            self.load(b'MZ')

    def test_hash_is_fnv1a_not_python_hash(self):
        self.assertEqual(fnv(''), 0x811c9dc5)
        self.assertEqual(fnv('hello'), 0x4f9f2cab)

    @unittest.skipUnless(importlib.util.find_spec('capstone') and importlib.util.find_spec('numpy'), 'optional disassembler dependencies')
    def test_instruction_xrefs_and_address_normalization(self):
        data = fixture()
        data.extend(b'\0' * 0x200)
        struct.pack_into('<H', data, 0x86, 2)
        struct.pack_into('<I', data, 0x98 + 56, 0x3000)
        struct.pack_into('<8sIIIIIIHHI', data, 0x98 + 240 + 40, b'.text', 0x200, 0x2000,
                         0x200, 0x1200, 0, 0, 0, 0, 0x60000020)
        struct.pack_into('<II', data, 0x98 + 112 + 3 * 8, 0x1380, 12)
        struct.pack_into('<III', data, 0x580, 0x2000, 0x2012, 0x13a0)
        data[0x5a0] = 1
        # lea rcx,[rip -> descriptor 0]; mov eax,<qualified hash>; call 0x2100; ret
        code = b'\x48\x8d\x0d' + struct.pack('<i', 0x1100 - 0x2007)
        code += b'\xb8' + struct.pack('<I', fnv('uint32'))
        code += b'\xe8' + struct.pack('<i', 0x2100 - 0x2011) + b'\xc3'
        data[0x1200:0x1212] = code
        pe = self.load(data)
        result = discover(pe, extract(pe), progress=lambda _: None)
        function = result['functions'][0]
        self.assertEqual(function['metadataReferences'][0]['typeIndex'], 0)
        self.assertEqual(function['hashCandidates'][0]['value'], fnv('uint32'))
        self.assertEqual(function['directBranches'][0]['targetRva'], 0x2100)
        self.assertFalse(function['callable'])
        struct.pack_into('<i', data, 0x120d, 0x2200 - 0x2011)
        other = self.load(data)
        normalized = discover(other, extract(other), progress=lambda _: None)['functions'][0]
        self.assertEqual(function['relocatedCodeSha256'], normalized['relocatedCodeSha256'])
        self.assertNotEqual(function['codeSha256'], normalized['codeSha256'])

    @unittest.skipUnless(importlib.util.find_spec('capstone'), 'optional disassembler dependency')
    def test_prefixed_sse_displacement_masks_all_four_address_bytes(self):
        import capstone as cs
        engine = cs.Cs(cs.CS_ARCH_X86, cs.CS_MODE_64)
        engine.detail = True
        normalized = []
        for raw in ('66 0f 6f 0d da 8e ec 00', '66 0f 6f 0d da f3 98 00'):
            code = bytearray.fromhex(raw)
            instruction = next(engine.disasm(bytes(code), 0x2000))
            mask_rip_displacement(code, instruction, 0x2000, instruction.disp)
            normalized.append(code)
        self.assertEqual(normalized[0], normalized[1])

    @unittest.skipUnless(importlib.util.find_spec('capstone') and importlib.util.find_spec('numpy'), 'optional disassembler dependencies')
    def test_chained_unwind_fragments_have_one_primary_function(self):
        data = fixture()
        data.extend(b'\0' * 0x200)
        struct.pack_into('<H', data, 0x86, 2)
        struct.pack_into('<I', data, 0x98 + 56, 0x3000)
        struct.pack_into('<8sIIIIIIHHI', data, 0x98 + 280, b'.text', 0x200, 0x2000,
                         0x200, 0x1200, 0, 0, 0, 0, 0x60000020)
        struct.pack_into('<II', data, 0x98 + 136, 0x1380, 24)
        struct.pack_into('<6I', data, 0x580, 0x2000, 0x2004, 0x13a0, 0x2004, 0x2010, 0x13b0)
        data[0x5a0] = 1
        data[0x5b0] = 0x21
        struct.pack_into('<III', data, 0x5b4, 0x2000, 0x2004, 0x13a0)
        data[0x1200:0x1210] = b'\x90' * 15 + b'\xc3'
        pe = self.load(data)
        result = discover(pe, extract(pe), progress=lambda _: None)
        self.assertEqual(len(result['functionGroups']), 1)
        self.assertEqual(result['functionGroups'][0]['ranges'], [0x2000, 0x2004])
        self.assertEqual(result['functionGroups'][0]['codeBytes'], 16)
        struct.pack_into('<I', data, 0x5bc, 0x13b0)
        with self.assertRaisesRegex(ValueError, 'cyclic'):
            self.load(data)

    def test_lookup_retains_every_collision_and_rejects_mixed_builds(self):
        spec = importlib.util.spec_from_file_location('inspect_runtime', TOOLS / 'inspect-runtime.py')
        inspect = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(inspect)
        pe = self.load(fixture())
        directory = pe.path.parent
        (directory / 'reflection.json').write_text(json.dumps(dict(image=pe.identity(), **extract(pe))))
        result = inspect.lookup(directory, '777', 'internalHash')
        self.assertEqual(result['matchCount'], 2)
        (directory / 'functions.json').write_text(json.dumps(dict(image={'sha256': 'wrong'})))
        with self.assertRaisesRegex(ValueError, 'mixes executable identities'):
            inspect.checked_reports(directory)


if __name__ == '__main__':
    unittest.main()
