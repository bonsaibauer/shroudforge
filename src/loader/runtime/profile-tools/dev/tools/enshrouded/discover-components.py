#!/usr/bin/env python3
"""Follow sparse ECS arrays to full engine registrations and entity managers."""
import argparse
import json
from pathlib import Path
from discovery.pe import PE
from discovery.live import Process
from discovery.components import discover


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--capture', type=Path, required=True)
    args = parser.parse_args()
    pe = PE(args.executable)
    reflection = json.loads((args.capture / 'reflection.json').read_text(encoding='utf-8'))
    live = json.loads((args.capture / 'live.json').read_text(encoding='utf-8'))
    if reflection['image'] != pe.identity() or live['image'] != pe.identity():
        raise ValueError('capture identity mismatch')
    process = Process(live['pid'], pe)
    try:
        if int(live['moduleBase'], 16) != process.base:
            raise ValueError('process restarted: capture again')
        report = discover(process, pe, reflection, live)
    finally:
        process.close()
    destination = args.capture / 'components.json'
    destination.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps([dict(count=r['count'], runtime=r['runtimeTypes'],
                          template=r['templateTypes'], templateOnly=r['templateOnly'],
                          managers=r['managers']) for r in report['registries']], indent=2))


if __name__ == '__main__':
    main()
