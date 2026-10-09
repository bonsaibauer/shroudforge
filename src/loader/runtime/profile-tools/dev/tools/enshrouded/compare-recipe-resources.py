"""Compare fresh inspect_attribute_resources exports; never modifies game assets."""
import argparse
from collections import Counter
from fractions import Fraction
import hashlib
import json
from pathlib import Path


def load(path):
    raw = path.read_bytes()
    data = json.loads(raw)
    recipes = {}
    for resource in data['resources']:
        if resource['qualifiedType'] != 'keen::RecipeRegistryResource':
            continue
        for recipe in resource['value']['recipes']:
            # Use the engine identifier, not an inferred/debug name, for matching.
            key = recipe['recipeId']['value']
            if key in recipes:
                raise ValueError(f'duplicate recipe ID {key} in {path}')
            recipes[key] = recipe
    if not recipes:
        raise ValueError(f'no RecipeRegistryResource recipes in {path}')
    return recipes, dict(export=str(path), exportSha256=hashlib.sha256(raw).hexdigest(),
                        version=data['version'], container=data['container'],
                        typeSource=data.get('type_source'))


def compare(left, right):
    common = sorted(left.keys() & right.keys())
    differences = []
    ratios = Counter()
    counts = Counter()
    for key in common:
        a, b = left[key], right[key]
        fields = [field for field in ('craftingDuration', 'input', 'output', 'debugName')
                  if a[field] != b[field]]
        if not fields:
            continue
        counts.update(fields)
        row = dict(recipeId=key, leftName=a['debugName'], rightName=b['debugName'],
                   fields={field: dict(left=a[field], right=b[field]) for field in fields})
        if 'craftingDuration' in fields:
            av, bv = a['craftingDuration']['value'], b['craftingDuration']['value']
            ratio = str(Fraction(bv, av)) if av else 'undefined (left zero)'
            row['rightToLeftDurationRatio'] = ratio
            ratios[ratio] += 1
        differences.append(row)
    return dict(commonRecipes=len(common), onlyLeft=sorted(left.keys() - right.keys()),
                onlyRight=sorted(right.keys() - left.keys()),
                changedFields=dict(counts), durationRatios=dict(ratios), differences=differences)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client', type=Path, required=True)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--client-backup', type=Path)
    parser.add_argument('--server-backup', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    client, ci = load(args.client)
    server, si = load(args.server)
    report = dict(schemaVersion=1, sources=dict(client=ci, server=si),
                  clientToServer=compare(client, server),
                  limitations=['Disk resource comparison, not a live-memory snapshot.',
                               'Backup files are not assumed to be original game data.',
                               'Recipe equality does not prove equal runtime settings or mod effects.'])
    for role, current, path in [('client', client, args.client_backup),
                                ('server', server, args.server_backup)]:
        if path:
            backup, identity = load(path)
            report['sources'][role + 'Backup'] = identity
            report[role + 'ToBackup'] = compare(current, backup)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({key: {k: v for k, v in value.items() if k != 'differences'}
                      for key, value in report.items() if 'To' in key}, indent=2))


if __name__ == '__main__':
    main()
