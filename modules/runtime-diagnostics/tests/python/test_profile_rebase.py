"""Independent evidence fixtures for complete, non-production profile rebases."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'diagnostics-jobs'))
from game_inspection.compare.prepare_profile_rebase import PRODUCTION_PROFILES, check_output_directory, leaves, prepare
from game_inspection.inspect.read_pe_image import PE


def executable(path, timestamp):
    data = bytearray(0x400)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 0x80)
    data[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<HHIIIHH', data, 0x84, 0x8664, 1, timestamp, 0, 0, 240, 0)
    struct.pack_into('<H', data, 0x98, 0x20b)
    struct.pack_into('<Q', data, 0x98 + 24, 0x140000000)
    struct.pack_into('<II', data, 0x98 + 56, 0x3000, 0x200)
    struct.pack_into('<8sIIIIIIHHI', data, 0x98 + 240, b'.text', 0x200, 0x1000,
                     0x200, 0x200, 0, 0, 0, 0, 0x60000020)
    path.parent.mkdir(parents=True)
    path.write_bytes(data)
    version = b'fixture'
    header = bytearray(24)
    header[:4] = b'KFC4'
    struct.pack_into('<II', header, 16, 8, len(version))
    path.with_suffix('.kfc').write_bytes(header + version)
    return PE(path).identity()


class ProfileRebaseTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.directory = Path(directory.name)
        self.source_exe = self.directory / 'source' / 'enshrouded.exe'
        self.target_exe = self.directory / 'target' / 'enshrouded.exe'
        self.source = executable(self.source_exe, 123)
        self.target = executable(self.target_exe, 456)
        self.source_capture = self.directory / 'source-capture'
        self.target_capture = self.directory / 'target-capture'
        self.capture(self.source_capture, self.source)
        self.capture(self.target_capture, self.target)
        self.profile = dict(schemaVersion=2, id='enshrouded-client-10', target='enshrouded.exe',
            kfcFormat=4, image={key: value for key, value in self.source.items() if key != 'target'},
            ownerLayout=dict(rootObjectRva=0x1100, ownerOffset=24),
            ecsWriteLayout=dict(enabled=True, threadOffset=48),
            resourceWriteLayout=dict(enabled=True, bufferCapacity=1024),
            provenance=dict(status='read-validated', resourceReadValidated=True,
                            limitations=['Baseline evidence is retained verbatim.']),
            extension=dict(emptyObject={}, emptyList=[], featureFlags=[True, False], name='preserve me'))
        self.profile_path = self.directory / '10.json'
        self.write(self.profile_path, self.profile)

    def write(self, path, value):
        path.write_text(json.dumps(value), encoding='utf-8')

    def capture(self, path, identity, roots=None):
        path.mkdir(exist_ok=True)
        self.write(path / 'reflection.json', dict(image=identity, kfcFormat=4, types=[]))
        if roots:
            self.write(path / 'functions.json', dict(image=identity,
                functionGroups=[dict(beginRva=root, ranges=[root], codeBytes=32,
                                     relocatedCodeSha256='a' * 64) for root in roots],
                functions=[dict(beginRva=root, endRva=root + 32, primaryBeginRva=root,
                                relocatedCodeSha256='b' * 64) for root in roots]))

    def run_prepare(self, **overrides):
        options = dict(source_profile=self.profile_path, source_capture=self.source_capture,
            target_capture=self.target_capture, target_executable=self.target_exe,
            build_id='20', output=self.directory / 'review')
        options.update(overrides)
        report = prepare(**options)
        candidate = json.loads((options['output'] / report['candidateFile']).read_text(encoding='utf-8'))
        return report, candidate

    def review_plan(self, changes):
        artifact = self.directory / 'owner-evidence.json'
        artifact.write_text('An exact-image observation fixture.', encoding='utf-8')
        evidence = dict(path=artifact.name, sha256=hashlib.sha256(artifact.read_bytes()).hexdigest(),
                        observation='The original owner field in the target is at offset 32.')
        for row in changes:
            row.setdefault('evidence', [deepcopy(evidence)])
        path = self.directory / 'reviewed-changes.json'
        self.write(path, dict(source=self.source, target=self.target, changes=changes))
        return path

    def test_copies_every_feature_and_flag_with_identity_only_diff(self):
        before = self.profile_path.read_bytes()
        report, candidate = self.run_prepare()
        expected = deepcopy(self.profile)
        expected['id'] = 'enshrouded-client-20'
        expected['image'].update({key: value for key, value in self.target.items() if key != 'target'})
        self.assertEqual(candidate, expected)
        self.assertEqual(self.profile_path.read_bytes(), before)
        self.assertEqual(report['baselineSha256Before'], report['baselineSha256After'])
        self.assertEqual({row['path'] for row in report['differences']}, {'/id', '/image/sha256', '/image/timestamp'})
        self.assertFalse(report['productionProfile'])
        self.assertFalse(report['approvalTransferred'])
        self.assertTrue(report['sourceProvenancePreserved'])
        self.assertEqual(report['preservedLeafCount'], len(dict(leaves(self.profile))))

    def test_rejects_capture_from_another_executable(self):
        self.capture(self.target_capture, self.source)
        with self.assertRaisesRegex(ValueError, 'exact source and target'):
            self.run_prepare()
        self.assertFalse((self.directory / 'review').exists())

    def test_rejects_client_server_substitution(self):
        target_exe = self.directory / 'server' / 'enshrouded_server.exe'
        server = executable(target_exe, 789)
        self.capture(self.target_capture, server)
        with self.assertRaisesRegex(ValueError, 'same Enshrouded process target'):
            self.run_prepare(target_executable=target_exe)

    def test_rejects_different_kfc_format(self):
        data = self.target_exe.with_suffix('.kfc').read_bytes()
        self.target_exe.with_suffix('.kfc').write_bytes(b'KFC3' + data[4:])
        with self.assertRaisesRegex(ValueError, 'same KFC format'):
            self.run_prepare()

    def test_static_unique_match_never_silently_changes_rva(self):
        self.capture(self.source_capture, self.source, [0x1100])
        self.capture(self.target_capture, self.target, [0x1200])
        report, candidate = self.run_prepare()
        field = next(row for row in report['numericReview'] if row['path'] == '/ownerLayout/rootObjectRva')
        self.assertEqual(field['staticCandidates']['candidateFunctionRva'], 0x1200)
        self.assertEqual(candidate['ownerLayout']['rootObjectRva'], 0x1100)
        self.assertEqual(field['status'], 'target-validation-required')
        self.assertFalse(field['staticCandidates']['callable'])

    def test_ambiguity_is_preserved_without_numeric_change(self):
        self.capture(self.source_capture, self.source, [0x1100])
        self.capture(self.target_capture, self.target, [0x1200, 0x1300])
        report, candidate = self.run_prepare()
        field = next(row for row in report['numericReview'] if row['path'] == '/ownerLayout/rootObjectRva')
        self.assertEqual(field['staticCandidates']['status'], 'ambiguous')
        self.assertEqual(len(field['staticCandidates']['rankedCandidates']), 2)
        self.assertEqual(candidate['ownerLayout']['rootObjectRva'], 0x1100)

    def test_numeric_changes_require_exact_images_and_hashed_evidence(self):
        plan = self.review_plan([dict(path='/ownerLayout/ownerOffset', before=24, after=32)])
        report, candidate = self.run_prepare(reviewed_changes=plan)
        self.assertEqual(candidate['ownerLayout']['ownerOffset'], 32)
        self.assertEqual(candidate['provenance'], self.profile['provenance'])
        row = next(row for row in report['differences'] if row['path'] == '/ownerLayout/ownerOffset')
        self.assertEqual(row['evidence']['method'], 'explicit-reviewed-numeric-change')
        self.assertFalse(row['evidence']['semanticsVerifiedByTool'])

    def test_wrong_review_image_is_rejected(self):
        plan = self.review_plan([dict(path='/ownerLayout/ownerOffset', before=24, after=32)])
        document = json.loads(plan.read_text())
        document['target'] = self.source
        self.write(plan, document)
        with self.assertRaisesRegex(ValueError, 'different source or target'):
            self.run_prepare(reviewed_changes=plan)

    def test_modified_evidence_is_rejected(self):
        plan = self.review_plan([dict(path='/ownerLayout/ownerOffset', before=24, after=32)])
        (self.directory / 'owner-evidence.json').write_text('Changed observation.', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'matching evidence bytes'):
            self.run_prepare(reviewed_changes=plan)

    def test_numeric_plan_cannot_change_flags_provenance_or_format(self):
        for pointer, before, after in [('/ecsWriteLayout/enabled', True, False),
                ('/provenance/resourceReadValidated', True, 1), ('/schemaVersion', 2, 1)]:
            with self.subTest(pointer=pointer):
                plan = self.review_plan([dict(path=pointer, before=before, after=after)])
                with self.assertRaises(ValueError):
                    self.run_prepare(reviewed_changes=plan)

    def test_existing_output_and_production_output_are_rejected(self):
        self.directory.joinpath('review').mkdir()
        with self.assertRaisesRegex(ValueError, 'existing files'):
            self.run_prepare()
        with self.assertRaisesRegex(ValueError, 'outside the production'):
            check_output_directory(PRODUCTION_PROFILES / 'client' / 'must-not-be-written')


if __name__ == '__main__':
    unittest.main()
