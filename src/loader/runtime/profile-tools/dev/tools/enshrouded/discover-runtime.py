#!/usr/bin/env python3
"""Reproducible EXE/optional live runtime inventory for client AND server."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import struct
import sys

from discovery.pe import PE
from discovery.reflection import extract
from discovery.functions import discover, profile_audit


def write(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')


def build_version(pe):
    # KFC data version is separate from executable identity; the EXE has no such string.
    path = pe.path.with_suffix('.kfc')
    if not path.is_file():
        return None
    with path.open('rb') as stream:
        header = stream.read(24)
        if len(header) != 24 or header[:4] != b'KFC3':
            raise ValueError('invalid adjacent KFC header')
        offset, count = struct.unpack_from('<II', header, 16)
        if not 1 <= count <= 4096 or 16 + offset + count > path.stat().st_size:
            raise ValueError('invalid KFC version location')
        stream.seek(16 + offset)
        return stream.read(count).decode('utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--profile', type=Path)
    parser.add_argument('--pid', type=int, help='optional read-only live scan; must be the selected EXE')
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--functions', action='store_true', help='disassemble every PE exception range (Capstone)')
    args = parser.parse_args()
    pe = PE(args.executable)
    reflection = extract(pe)
    version = build_version(pe)
    profile = json.loads(args.profile.read_text(encoding='utf-8-sig')) if args.profile else None
    identity = pe.identity()
    existing = args.out / 'reflection.json'
    if existing.is_file() and json.loads(existing.read_text(encoding='utf-8'))['image'] != identity:
        raise ValueError('output directory belongs to another executable; choose a new --out directory')
    print(f'{identity["target"]}: {version or "unknown build string"}, {reflection["count"]} reflected types', flush=True)
    args.out.mkdir(parents=True, exist_ok=True)
    # Each report carries identity. Never infer target/build from the selected profile's filename.
    common = dict(schemaVersion=1, image=identity, version=version, versionSource='adjacent-kfc-data',
                  capturedAt=datetime.now(timezone.utc).isoformat())
    write(args.out / 'reflection.json', dict(common, **reflection))
    audit = profile_audit(pe, profile)
    write(args.out / 'profile-audit.json', dict(common, audit=audit))
    summary = dict(common, reflectionTypes=reflection['count'],
                   reflectedFields=sum(len(t['structFields']) for t in reflection['types']),
                   compilerFunctionRanges=len(pe.functions), fullRuntimeResolved=False,
                   profileAudit=audit)
    if args.pid:
        from discovery.live import capture
        live = capture(pe, reflection, args.pid, profile, progress=lambda text: print(text, flush=True))
        write(args.out / 'live.json', dict(common, **live))
        from discovery.live import Process
        from discovery.components import discover as discover_components
        process = Process(args.pid, pe)
        try:
            components = discover_components(process, pe, reflection, live,
                progress=lambda text: print(text, flush=True))
        finally:
            process.close()
        write(args.out / 'components.json', dict(common, **components))
        anchored = [t for t in live['tables'] if t['status'] in
                    ('profile-anchored-component-table', 'allocated-ecs-table-candidate')]
        summary['live'] = dict(verifiedReflectionTypes=live['verifiedReflectionTypes'],
                              skippedPages=len(live['skippedPages']), componentTables=[
                                  {k: t[k] for k in ('address', 'count', 'status', 'discovery', 'inspectedSlots',
                                                    'profileMatches', 'stableReadback', 'rva') if k in t}
                                  for t in anchored])
        summary['live']['registrations'] = [
            {k: r[k] for k in ('count', 'runtimeTypes', 'templateTypes', 'templateOnly', 'managers')}
            for r in components['registries']]
    if args.functions:
        functions = discover(pe, reflection, progress=lambda text: print(text, flush=True))
        write(args.out / 'functions.json', dict(common, **functions))
        summary['codeCoverage'] = functions['coverage']
    write(args.out / 'summary.json', summary)
    print(json.dumps({k: v for k, v in summary.items() if k != 'profileAudit'}, indent=2))
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, ImportError) as error:
        print(f'discovery failed: {error}', file=sys.stderr)
        sys.exit(2)
