"""Independent fixtures for the live registration reader; no game process needed."""
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from discovery.components import read_registry
from discovery.reflection import fnv


class Memory:
    base = 0x10000
    def __init__(self):
        self.data = bytearray(0x30000)

    def read(self, address, size):
        if address < self.base or address + size > self.base + len(self.data):
            raise OSError('outside fixture')
        return bytes(self.data[address - self.base:address - self.base + size])

    def put(self, address, fmt, *values):
        struct.pack_into(fmt, self.data, address - self.base, *values)


class Image:
    def __init__(self, memory): self.memory = memory
    def read(self, rva, size): return self.memory.read(self.memory.base + rva, size)
    def executable(self, rva): return 0x6000 <= rva < 0x7000


def fixture():
    memory = Memory()
    owner, records, sizes, pointers, callbacks = 0x28000, 0x20000, 0x29000, 0x29200, 0x29400
    memory.put(owner + 8, '<QQQ', records, 16, 32)
    memory.put(owner + 232, '<QQ', sizes, 16)
    memory.put(owner + 256, '<QQ', pointers, 16)
    memory.put(owner + 280, '<QQ', callbacks, 16)
    types = []
    for i in range(16):
        name = f'keen::ecs::A{i}'
        text = (name + '\0').encode()
        text_addr = memory.base + 0x100 + i * 128
        memory.data[text_addr - memory.base:text_addr - memory.base + len(text)] = text
        metadata = memory.base + 0x2000 + i * 0x90
        ty = dict(index=i, qualifiedName=name, qualifiedHash=fnv(name), metadataRva=metadata - memory.base, size=4)
        types.append(ty)
        runtime, template = (metadata, 0) if i else (0, metadata)
        memory.put(records + i * 256, '<7Q', text_addr, len(text), text_addr, len(text), fnv(name), runtime, template)
        # Upper 16 bits at +56 are flags, not part of the component's size.
        memory.put(records + i * 256 + 56, '<HHI', 4 if runtime else 0, 0x100, 1)
        memory.put(sizes + i * 2, '<H', 4 if runtime else 0)
        memory.put(pointers + i * 8, '<Q', runtime)
    memory.put(callbacks + 40, '<Q', memory.base + 0x6000)
    return memory, Image(memory), dict(types=types), owner


class ComponentRegistryTest(unittest.TestCase):
    def test_template_only_is_resolved_without_inventing_entity_storage(self):
        memory, pe, reflection, owner = fixture()
        report = read_registry(memory, pe, reflection, owner)
        self.assertEqual((report['count'], report['runtimeTypes'], report['templateOnly']), (16, 15, 1))
        self.assertEqual(report['rows'][0]['storage'], 'template-only')
        self.assertIsNone(report['rows'][0]['runtimeType'])
        self.assertEqual(report['rows'][1]['runtimeSize'], 4)
        self.assertEqual(report['rows'][1]['storageFlagsBits'], 0x100)
        self.assertFalse(report['rows'][1]['callbacks'][0]['callable'])

    def test_hash_mismatch_is_not_a_mapping(self):
        memory, pe, reflection, owner = fixture()
        memory.put(0x20000 + 256 + 32, '<Q', 123)
        with self.assertRaisesRegex(ValueError, 'identity/storage mismatch'):
            read_registry(memory, pe, reflection, owner)

    def test_disagreement_with_parallel_storage_arrays_fails_closed(self):
        memory, pe, reflection, owner = fixture()
        memory.put(0x29000 + 2, '<H', 32)
        with self.assertRaisesRegex(ValueError, 'identity/storage mismatch'):
            read_registry(memory, pe, reflection, owner)

    def test_data_pointer_cannot_be_published_as_native_callback(self):
        memory, pe, reflection, owner = fixture()
        memory.put(0x29400 + 40, '<Q', memory.base + 0x3000)
        with self.assertRaisesRegex(ValueError, 'outside executable code'):
            read_registry(memory, pe, reflection, owner)

    def test_truncated_or_mismatched_array_headers_are_rejected(self):
        memory, pe, reflection, owner = fixture()
        memory.put(owner + 264, '<Q', 15)
        with self.assertRaisesRegex(ValueError, 'array headers mismatch'):
            read_registry(memory, pe, reflection, owner)


if __name__ == '__main__': unittest.main()
