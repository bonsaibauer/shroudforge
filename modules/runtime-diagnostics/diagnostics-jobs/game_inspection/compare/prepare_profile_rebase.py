"""Prepare a complete same-target profile copy and an explicit rebase review.

Static matches are suggestions only. The candidate preserves the baseline's
features and provenance and is never written into the production profile tree.
"""
import argparse
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from game_inspection.analyze.index_captured_functions import FunctionIndex
from game_inspection.analyze.read_capture_reports import checked_reports
from game_inspection.capture.discover_runtime_build import build_version
from game_inspection.compare.compare_runtime_builds import compare
from game_inspection.compare.rank_cross_build_functions import rank_operation, unique_anchors
from game_inspection.inspect.read_pe_image import PE


PRODUCTION_PROFILES = Path(__file__).resolve().parents[5] / 'modules/game-runtime/profiles'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def leaves(value, pointer=''):
    if isinstance(value, dict):
        for key, child in value.items():
            escaped = key.replace('~', '~0').replace('/', '~1')
            yield from leaves(child, pointer + '/' + escaped)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from leaves(child, pointer + '/' + str(index))
    else:
        yield pointer, value


def pointer_parent(value, pointer):
    if not isinstance(pointer, str) or not pointer.startswith('/'):
        raise ValueError('change path must be a JSON pointer to an existing numeric leaf')
    parts = []
    for token in pointer[1:].split('/'):
        if re.search(r'~(?![01])', token):
            raise ValueError('invalid JSON pointer escape')
        parts.append(token.replace('~1', '/').replace('~0', '~'))
    for token in parts[:-1]:
        value = value[int(token)] if isinstance(value, list) else value[token]
    token = int(parts[-1]) if isinstance(value, list) else parts[-1]
    return value, token


def apply_reviewed_changes(candidate, changes_path, source, target):
    """Bind explicit numeric edits to both images and immutable evidence files.

    This checks the review plan's consistency, not the truth of an observation
    or engine semantics. The resulting file remains a non-production candidate.
    """
    if changes_path is None:
        return {}
    plan = json.loads(changes_path.read_text(encoding='utf-8-sig'))
    if plan.get('source') != source or plan.get('target') != target:
        raise ValueError('reviewed changes belong to different source or target images')
    reviewed = {}
    for row in plan.get('changes', []):
        pointer = row['path']
        if pointer in reviewed:
            raise ValueError('duplicate numeric change: ' + pointer)
        if pointer in ('/id', '/target', '/schemaVersion', '/kfcFormat') or pointer.startswith(('/image/', '/provenance/')):
            raise ValueError('identity, profile format and provenance cannot be edited by a numeric review plan')
        try:
            owner, key = pointer_parent(candidate, pointer)
            before = owner[key]
        except (KeyError, IndexError, TypeError, ValueError) as error:
            raise ValueError('change path is not an existing numeric leaf: ' + pointer) from error
        after = row['after']
        if type(before) is not int or type(after) is not int or after < 0:
            raise ValueError('review plans may change nonnegative integer leaves only: ' + pointer)
        if type(row.get('before')) is not int or row['before'] != before:
            raise ValueError('numeric change does not match the baseline: ' + pointer)
        evidence = []
        for artifact in row.get('evidence', []):
            path = (changes_path.parent / artifact['path']).resolve()
            observation = artifact.get('observation', '').strip()
            if not observation or not path.is_file() or artifact.get('sha256') != digest(path.read_bytes()):
                raise ValueError('numeric change requires matching evidence bytes and a concrete observation: ' + pointer)
            evidence.append(dict(path=str(path), sha256=artifact['sha256'], observation=observation))
        if not evidence:
            raise ValueError('numeric change has no evidence: ' + pointer)
        owner[key] = after
        reviewed[pointer] = dict(method='explicit-reviewed-numeric-change', evidence=evidence,
                                 semanticsVerifiedByTool=False)
    return reviewed


def rank_profile_rvas(profile, source_capture, target_capture):
    left, left_functions, _, _ = checked_reports(source_capture)
    right, right_functions, _, _ = checked_reports(target_capture)
    if not left_functions or not right_functions:
        return {}
    source = FunctionIndex(left, left_functions)
    target = FunctionIndex(right, right_functions)
    anchors, methods, _ = unique_anchors(source, target)
    return {pointer: rank_operation(dict(id=pointer, kind='profile-rva', functionRva=value),
                source, target, anchors, methods, [])
            for pointer, value in leaves(profile)
            if type(value) is int and pointer.rsplit('/', 1)[-1].endswith('Rva')}


def check_output_directory(output):
    output = output.resolve()
    if output.is_relative_to(PRODUCTION_PROFILES.resolve()):
        raise ValueError('rebase candidates must stay outside the production profile tree')
    if output.exists():
        raise ValueError('choose a new output directory, existing files are never overwritten')
    return output


def prepare(source_profile, source_capture, target_capture, target_executable,
            build_id, output, reviewed_changes=None):
    output = check_output_directory(output)
    if not re.fullmatch(r'[1-9][0-9]*', str(build_id)):
        raise ValueError('build ID must be a positive numeric Steam build ID')
    original = source_profile.read_bytes()
    profile = json.loads(original.decode('utf-8-sig'))
    target_pe = PE(target_executable)
    target = target_pe.identity()
    source = dict(target=profile['target'], **{key: profile['image'][key] for key in ('sha256', 'timestamp', 'size')})
    if not re.fullmatch(r'[0-9a-f]{64}', source.get('sha256', '')):
        raise ValueError('the baseline requires an exact SHA-256 image identity')
    if source['target'] not in ('enshrouded.exe', 'enshrouded_server.exe') or source['target'] != target['target']:
        raise ValueError('source and target must be the same Enshrouded process target')
    left, _, _, _ = checked_reports(source_capture)
    right, _, _, _ = checked_reports(target_capture)
    if left['image'] != source or right['image'] != target:
        raise ValueError('captures and profile must belong to the exact source and target images')
    _, target_format = build_version(target_pe)
    if target_format != profile['kfcFormat'] or left.get('kfcFormat') != target_format or right.get('kfcFormat') != target_format:
        raise ValueError('source profile, captures and adjacent target KFC must use the same KFC format')
    if source == target:
        raise ValueError('source and target are the same executable, a rebase is unnecessary')
    candidate = deepcopy(profile)
    kind = 'client' if target['target'] == 'enshrouded.exe' else 'server'
    candidate['id'] = f'enshrouded-{kind}-{build_id}'
    candidate['image'].update({key: target[key] for key in ('sha256', 'timestamp', 'size')})
    reviewed = apply_reviewed_changes(candidate, reviewed_changes, source, target)
    source_leaves, candidate_leaves = dict(leaves(profile)), dict(leaves(candidate))
    if source_leaves.keys() != candidate_leaves.keys():
        raise ValueError('rebase must preserve every baseline field')
    identity_paths = {'/id', '/image/sha256', '/image/timestamp', '/image/size'}
    differences = []
    for pointer, before in source_leaves.items():
        after = candidate_leaves[pointer]
        if type(before) == type(after) and before == after:
            continue
        if pointer not in identity_paths and pointer not in reviewed:
            raise ValueError('unreviewed profile change: ' + pointer)
        differences.append(dict(path=pointer, before=before, after=after,
            evidence=reviewed.get(pointer, dict(method='exact-target-image-identity'))))
    comparison = compare(source_capture, target_capture, source_profile=source_profile,
                         target_executable=target_executable)
    rvas = rank_profile_rvas(profile, source_capture, target_capture)
    numeric_review = []
    for pointer, value in source_leaves.items():
        if type(value) is not int or pointer in identity_paths or pointer in ('/schemaVersion', '/kfcFormat') or pointer.startswith('/provenance/'):
            continue
        row = dict(path=pointer, baseline=value, candidate=candidate_leaves[pointer],
                   status='review-plan-supplied' if pointer in reviewed else 'target-validation-required')
        if pointer in reviewed:
            row['review'] = reviewed[pointer]
        if pointer in rvas:
            row['staticCandidates'] = rvas[pointer]
        numeric_review.append(row)
    unresolved = [dict(path=row['path'], reason='copied-baseline-value-has-no-target-review')
                  for row in numeric_review if row['status'] == 'target-validation-required']
    unresolved.extend(dict(scope=scope, reason='not-established-by-copy-or-static-ranking') for scope in (
        'exact-image-original-owner-and-lifetime-chains', 'scene-authority-and-thread-gates',
        'target-client-or-server-startup', 'native-operation-arguments-and-live-effects'))
    if digest(source_profile.read_bytes()) != digest(original):
        raise ValueError('the baseline changed during the rebase, no candidate was written')
    candidate_bytes = (json.dumps(candidate, indent=2, ensure_ascii=False) + '\n').encode('utf-8')
    report = dict(schemaVersion=1, kind='profile-rebase-review', productionProfile=False,
        source=source, target=target, baselinePath=str(source_profile.resolve()),
        baselineSha256Before=digest(original), baselineSha256After=digest(source_profile.read_bytes()),
        candidateFile=f'{build_id}.candidate.json', candidateSha256=digest(candidate_bytes),
        completeBaselineCopy=True, preservedLeafCount=len(source_leaves),
        sourceProvenancePreserved=True, approvalTransferred=False, semanticsVerified=False,
        differences=differences, numericReview=numeric_review, comparison=comparison,
        unresolved=unresolved, limitations=[
            'Copied provenance describes baseline evidence and is not approval for the target executable.',
            'Static function rankings are never applied as profile values.',
            'A numeric review plan binds supplied observations to exact images and evidence bytes, but the tool cannot verify their semantic truth.',
            'All unchanged numeric fields and the original ownership, authority and lifetime contracts still require target validation.',
            'A complete candidate outside production profiles is not an executable runtime compatibility approval.'])
    output.mkdir(parents=True, exist_ok=False)
    (output / report['candidateFile']).write_bytes(candidate_bytes)
    (output / 'rebase-review.json').write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source_profile', type=Path)
    parser.add_argument('source_capture', type=Path)
    parser.add_argument('target_capture', type=Path)
    parser.add_argument('target_executable', type=Path)
    parser.add_argument('--build-id', required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--reviewed-changes', type=Path,
        help='exact-image JSON plan with numeric leaf edits and hashed evidence artifacts')
    args = parser.parse_args()
    report = prepare(args.source_profile, args.source_capture, args.target_capture,
        args.target_executable, args.build_id, args.out, args.reviewed_changes)
    print(f'Complete candidate and {len(report["unresolved"])} unresolved checks saved to {args.out.resolve()}')
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f'profile rebase failed: {error}', file=sys.stderr)
        sys.exit(2)
