#!/usr/bin/env python3
"""Query captured registries by hash/name and compare executable builds."""
import argparse
import json
from pathlib import Path
import sys
from collections import Counter


def read(directory, name, optional=False):
    path = directory / name
    if optional and not path.is_file():
        return None
    return json.loads(path.read_text(encoding='utf-8'))


def checked_reports(directory):
    reflection = read(directory, 'reflection.json')
    functions = read(directory, 'functions.json', True)
    live = read(directory, 'live.json', True)
    audit = read(directory, 'profile-audit.json', True)
    for report in (functions, live, audit):
        if report and report['image'] != reflection['image']:
            raise ValueError('capture directory mixes executable identities')
    return reflection, functions, live, audit


def lookup(directory, query, hash_kind):
    reflection, functions, live, audit = checked_reports(directory)
    try:
        value = int(query, 0)
        if not 0 <= value <= 0xffffffff:
            raise ValueError('hash must be an unsigned 32-bit integer')
        matches = [t for t in reflection['types'] if t[hash_kind] == value]
    except ValueError:
        matches = [t for t in reflection['types'] if t['qualifiedName'] == query]
    result = dict(image=reflection['image'], query=query, matchCount=len(matches), matches=[])
    for ty in matches:
        index = ty['index']
        code = []
        for fn in (functions or {}).get('functions', []):
            refs = [r for r in fn['metadataReferences'] if r['typeIndex'] == index]
            hashes = [r for r in fn['hashCandidates'] if any(c['typeIndex'] == index for c in r['candidates'])]
            if refs or hashes:
                code.append(dict(id=fn['id'], beginRva=fn['beginRva'], endRva=fn['endRva'],
                                 metadataReferences=refs, hashCandidates=hashes, callable=False))
        tables = [dict(address=t['address'], status=t['status'], index=row['index'],
                       stableReadback=t['stableReadback'])
                  for t in (live or {}).get('tables', []) for row in t['rows'] if row['typeIndex'] == index]
        result['matches'].append(dict(type=ty, liveTables=tables, codeReferences=code))
    return result


def layout(ty, types):
    def name(index):
        return types[index]['qualifiedName'] if index is not None else None
    return dict(size=ty['size'], alignment=ty['alignment'], primitive=ty['primitiveType'],
                flags=ty['flagsBits'], innerType=name(ty.get('innerType')),
                fields={key: dict(type=name(f['type']), offset=f['dataOffset']) for key, f in ty['structFields'].items()},
                enums=ty['enumFields'])


def compare(source, target):
    left, lf, _, audit = checked_reports(source)
    right, rf, _, _ = checked_reports(target)
    a = {t['qualifiedName']: t for t in left['types']}
    b = {t['qualifiedName']: t for t in right['types']}
    common = sorted(a.keys() & b.keys())
    changed = [dict(name=name, source=layout(a[name], left['types']), target=layout(b[name], right['types']))
               for name in common if layout(a[name], left['types']) != layout(b[name], right['types'])]
    result = dict(source=left['image'], target=right['image'],
                  addedTypes=sorted(b.keys() - a.keys()), removedTypes=sorted(a.keys() - b.keys()),
                  changedLayouts=changed, unchangedLayouts=len(common) - len(changed), operationCandidates=[])
    if lf and rf and audit and audit['audit']:
        fingerprints = {}
        for fn in rf.get('functionGroups', []):
            digest = fn.get('relocatedCodeSha256')
            if digest:
                fingerprints.setdefault(digest, []).append(fn['beginRva'])
        source_functions = {f['beginRva']: f for f in lf.get('functionGroups', [])}
        target_functions = {f['beginRva']: f for f in rf.get('functionGroups', [])}
        left_ranges = {f['beginRva']: f for f in lf['functions']}
        right_ranges = {f['beginRva']: f for f in rf['functions']}

        def callees(group, ranges):
            return {ranges.get(branch['targetRva'], {}).get('primaryBeginRva', branch['targetRva'])
                    for rva in group['ranges'] for branch in ranges[rva]['directBranches'] if branch['kind'] == 'call'}

        called_by = {}
        for fn in target_functions.values():
            for callee in callees(fn, right_ranges):
                called_by.setdefault(callee, set()).add(fn['beginRva'])
        for operation in audit['audit']['operations']:
            rva = operation.get('functionRva')
            fn = source_functions.get(rva)
            if not fn:
                continue
            matches = fingerprints.get(fn.get('relocatedCodeSha256'), []) if fn['codeBytes'] >= 32 else []
            topology = []
            if not matches:
                translated = set()
                for callee in callees(fn, left_ranges):
                    callee_fn = source_functions.get(callee)
                    if callee_fn and callee_fn['codeBytes'] >= 32:
                        targets = fingerprints.get(callee_fn.get('relocatedCodeSha256'), [])
                        if len(targets) == 1:
                            translated.add(targets[0])
                votes = Counter(caller for callee in translated for caller in called_by.get(callee, []))
                ranked = sorted(votes, key=lambda candidate: (-votes[candidate],
                    abs(target_functions[candidate]['codeBytes'] - fn['codeBytes']), candidate))
                for candidate in ranked[:10]:
                    if votes[candidate] < 2:
                        continue
                    topology.append(dict(targetRva=candidate, matchedCallees=votes[candidate],
                                         translatableCallees=len(translated),
                                         codeBytes=target_functions[candidate]['codeBytes'],
                                         method='shared-unique-callee-fingerprints', semanticStatus='unverified'))
            result['operationCandidates'].append(dict(id=operation['id'], sourceRva=rva, targetRvas=matches,
                                                       sourceCodeBytes=fn['codeBytes'], sourceRanges=fn['ranges'],
                                                       method='chained-unwind-group-bytes-with-address-operands-masked',
                                                       callGraphCandidates=topology,
                                                       semanticStatus='unverified', callable=False))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    query = sub.add_parser('lookup')
    query.add_argument('capture', type=Path)
    query.add_argument('query')
    query.add_argument('--hash-kind', choices=['qualifiedHash', 'internalHash', 'nameHash', 'impactHash'], default='qualifiedHash')
    diff = sub.add_parser('compare')
    diff.add_argument('source', type=Path)
    diff.add_argument('target', type=Path)
    for command in (query, diff):
        command.add_argument('--out', type=Path)
    args = parser.parse_args()
    result = lookup(args.capture, args.query, args.hash_kind) if args.command == 'lookup' else compare(args.source, args.target)
    text = json.dumps(result, indent=2, ensure_ascii=False) + '\n'
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding='utf-8')
        print(args.out)
    else:
        print(text)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError) as error:
        print(f'inspection failed: {error}', file=sys.stderr)
        sys.exit(2)
