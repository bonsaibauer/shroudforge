"""Read-only FactoryStation/Inventory capture using a revalidated discovery registry.

Requires a components.json discovery export (or a registry list) and its reflection.
Snapshots are observations, not an atomic world snapshot or a server acknowledgement.
"""
import argparse
import datetime
import json
import struct
from pathlib import Path

from discovery.components import read_registry, validate_manager
from discovery.live import Process
from discovery.pe import PE


def capture(process, pe, reflection, registries):
    types = {t['qualifiedName']: t for t in reflection['types']}
    stations, seen, proofs = [], set(), []

    def unpack(address, fmt):
        return struct.unpack(fmt, process.read(address, struct.calcsize(fmt)))

    for previous in registries:
        registry = read_registry(process, pe, reflection, int(previous['ownerAddress'], 16))
        indices = {r['qualifiedName']: r for r in registry['rows']}
        for manager in previous.get('managers', []):
            address = int(manager['address'], 16)
            proofs.append(validate_manager(process, address, registry))
            count = unpack(address + 344, '<Q')[0]
            table = unpack(address + 392, '<Q')[0]
            pointers = unpack(table, '<' + 'Q' * count)
            views = {}
            manager_stations = []
            for entity in pointers:
                if not entity or entity in seen:
                    continue
                seen.add(entity)
                try:
                    header = process.read(entity + 16, 36)
                    eid, generation, layout, storage, definition, row = struct.unpack('<IIQQQI', header)
                    if not eid or not layout or not storage or row > 1 << 24:
                        continue
                    views.setdefault(eid, []).append((entity, header))

                    def component(name):
                        entry = indices.get(name)
                        if not entry or not entry['runtimeSize']:
                            return None
                        index, size = entry['index'], entry['runtimeSize']
                        if not unpack(layout + (index // 64) * 8, '<Q')[0] & (1 << (index % 64)):
                            return None
                        offset = unpack(layout + 132 + index * 2, '<H')[0]
                        stride = unpack(layout + 2692 + index * 2, '<H')[0]
                        if stride != size:
                            raise ValueError('component stride disagrees with live registry')
                        ptr = storage + offset + row * stride
                        return ptr, process.read(ptr, size)

                    station = component('keen::ecs::FactoryStation')
                    if not station:
                        continue
                    ptr, data = station
                    fields = types['keen::ecs::FactoryStation']['structFields']

                    def field(name, fmt):
                        return struct.unpack_from(fmt, data, fields[name]['dataOffset'])[0]

                    record = dict(entityId=eid, generation=generation, address=hex(ptr),
                                  state=field('state', '<B'), runningRecipe=field('runningRecipe', '<I'),
                                  nextRecipe=field('nextRecipe', '<I'), recipeStart=field('recipeStart', '<q'),
                                  recipePauseDuration=field('recipePauseDuration', '<q'))
                    inventory = component('keen::ecs::Inventory')
                    if inventory:
                        # Eight reflected ItemStack storage records; retain the third word unnamed.
                        if len(inventory[1]) != 96:
                            raise ValueError('unsupported Inventory layout')
                        record['inventory'] = [dict(itemId=a, count=b, extra=c)
                                               for a, b, c in struct.iter_unpack('<III', inventory[1])]
                    setup = component('keen::ecs::InventorySetup')
                    if setup:
                        offset = types['keen::ecs::InventorySetup']['structFields']['linksEntities']['dataOffset']
                        record['inventoryLinks'] = [value for value in struct.unpack_from('<16I', setup[1], offset) if value]
                    # Entity identity and station state must survive the read; no atomicity claimed.
                    if process.read(entity + 16, 36) != header or process.read(ptr, len(data)) != data:
                        continue
                    stations.append(record)
                    manager_stations.append(record)
                except (OSError, ValueError):
                    continue
            for record in manager_stations:
                record['linkedInventories'] = []
                for linked_id in record.get('inventoryLinks', []):
                    candidates = views.get(linked_id, [])
                    if len(candidates) != 1:
                        record.setdefault('unresolvedInventoryLinks', []).append(linked_id)
                        continue
                    entity, header = candidates[0]
                    try:
                        eid, generation, layout, storage, definition, row = struct.unpack('<IIQQQI', header)
                        inventory = component('keen::ecs::Inventory')
                        if not inventory or len(inventory[1]) != 96 or process.read(entity + 16, 36) != header:
                            raise ValueError('linked inventory changed or unavailable')
                        record['linkedInventories'].append(dict(entityId=eid, generation=generation,
                            slots=[dict(itemId=a, count=b, extra=c)
                                   for a, b, c in struct.iter_unpack('<III', inventory[1])]))
                    except (OSError, ValueError):
                        record.setdefault('unresolvedInventoryLinks', []).append(linked_id)
    return dict(schemaVersion=1, capturedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                image=pe.identity(), managers=proofs, stations=stations, consistentSnapshot=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--pid', type=int, required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--components', type=Path)
    source.add_argument('--manager', type=lambda value: int(value, 0), action='append')
    parser.add_argument('--reflection', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    pe = PE(args.exe)
    reflection = json.loads(args.reflection.read_text(encoding='utf-8'))
    if reflection['image']['sha256'] != pe.sha256:
        raise ValueError('reflection executable identity mismatch')
    process = Process(args.pid, pe)
    try:
        if args.components:
            components = json.loads(args.components.read_text(encoding='utf-8'))
            registries = components if isinstance(components, list) else components['registries']
        else:
            registries = []
            for manager in args.manager:
                owner = struct.unpack('<Q', process.read(manager, 8))[0]
                registry = read_registry(process, pe, reflection, owner)
                registry['managers'] = [validate_manager(process, manager, registry)]
                registries.append(registry)
        report = capture(process, pe, reflection, registries)
    finally:
        process.close()
    report['pid'] = args.pid
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(f"Read {len(report['stations'])} factory stations; no game memory or files changed")


if __name__ == '__main__':
    main()
