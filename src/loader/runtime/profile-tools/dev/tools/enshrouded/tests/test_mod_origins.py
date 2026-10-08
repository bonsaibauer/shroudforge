import copy
import importlib.util
from pathlib import Path
import sys
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
spec = importlib.util.spec_from_file_location('origins', TOOLS / 'audit-mod-origins.py')
origins = importlib.util.module_from_spec(spec)
spec.loader.exec_module(origins)


class ModOriginsTest(unittest.TestCase):
    def test_rejects_the_old_server_flight_rip_displacement(self):
        entry = dict(kind='detour', returnRel32Offset=9, modifier=dict(id='override_rotation_constant'),
                     payload=list(bytes.fromhex('f3 0f 10 05 05 00 00 00 e9 00 00 00 00 c3 f5 c8 bf')),
                     inlineReferences=[dict(displacementOffset=4, nextInstructionOffset=8, dataOffset=13, dataSize=4)])
        self.assertTrue(origins.validate_payload(entry))
        broken = copy.deepcopy(entry)
        broken['payload'][4:8] = bytes.fromhex('63 20 af 00')
        with self.assertRaisesRegex(AssertionError, 'inline constant'):
            origins.validate_payload(broken)

    def test_health_preservation_is_not_the_old_addition(self):
        entry = dict(kind='detour', returnRel32Offset=8, modifier=dict(id='preserve_health_on_fall'),
                     payload=list(bytes.fromhex('90 90 90 90 48 8b cb e9 00 00 00 00')))
        self.assertTrue(origins.validate_payload(entry))
        entry['payload'][:4] = bytes.fromhex('45 01 0c 88')
        with self.assertRaisesRegex(AssertionError, 'not add'):
            origins.validate_payload(entry)


if __name__ == '__main__':
    unittest.main()
