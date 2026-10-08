#!/usr/bin/env python3
"""Export the complete observed type/component/code schema and a small build index."""
import argparse
import json
import os
from pathlib import Path

from discovery.pe import PE


def export(pe, capture):
    def read(name):
        return json.loads((capture / name).read_text(encoding='utf-8'))
    reflection = read('reflection.json')
    components = read('components.json')
    verified = read('provider-verification.json')
    native = read('provider-verification.json.functions.json')
    if reflection['image'] != pe.identity() or components['image'] != pe.identity():
        raise ValueError('capture does not belong to the selected executable')
    if any(native['image'][key] != pe.identity()[key] for key in ('timestamp', 'size')):
        raise ValueError('native snapshot belongs to another PE layout')
    entries = {entry['rva']: dict(entry) for entry in native['entries']}
    adapter_path = capture / 'provider-verification.json.adapters.json'
    adapters = {entry['rva']: entry for entry in read(adapter_path.name)['adapters']} if adapter_path.is_file() else {}
    differential_path = capture / 'adapter-verification.json'
    differential = read(differential_path.name) if differential_path.is_file() else None
    if differential and differential['image'] != pe.identity():
        raise ValueError('adapter verification belongs to another executable')
    verified_adapters = {entry['rva'] for entry in differential['results'] if entry['bufferEffectsMatch']} if differential else set()
    for entry in entries.values():
        entry.update(name=f"unclear_{entry['rva']:08x}", owners=[])
    callback_rvas = set()
    for row in verified['entries']:
        for callback in row['callbacks']:
            rva = callback['function_rva']
            if not pe.executable(rva):
                raise ValueError('callback outside executable code')
            callback_rvas.add(rva)
            entry = entries.setdefault(rva, dict(rva=rva, id=f'native:{rva}', name=f'unclear_{rva:08x}', ranges=[], owners=[], code_bytes=None))
            entry['owners'].append(dict(qualified_name=row['qualified_name'], qualified_hash=row['qualified_hash'],
                                        registration_index=row['index'], origin=callback['origin'], slot_offset=callback['slot_offset']))
            entry['address_evidence'] = 'live-engine-registration'
            if rva in adapters:
                plan = adapters[rva]['buffer_transform']
                code = bytes.fromhex(callback['code_hex'])[:plan['code_length']]
                if code != pe.read(rva, len(code)) or rva not in verified_adapters:
                    raise ValueError('adapter lacks matching code and independent differential verification')
                entry.update(buffer_transform=plan, execution='owned-buffer-copy-interpreter')
    for entry in entries.values():
        entry.update(key=pe.sha256 + '/' + entry['name'], provisional=True, native_callable=False,
                     callable='buffer_transform' in entry,
                     signature=dict(native_arguments=None, native_return=None, complete=False, calling_convention='windows-x64'),
                     validation=dict(address=entry.get('address_evidence'), type_ownership=bool(entry['owners']),
                                     native_signature=False, engine_context=False, gameplay_effects=False,
                                     owned_buffer_effects='buffer_transform' in entry))
        entry['reason'] = None if entry['callable'] else 'native arguments, engine context and effects are unresolved; no verified adapter'
    counts = dict(types=reflection['count'], registrations=verified['runtime'] + verified['templateOnly'],
                  runtimeLayouts=verified['runtime'], templateOnly=verified['templateOnly'], codeCandidates=len(entries),
                  unwindGroups=native['unwind_count'], distinctRegisteredCallbacks=len(callback_rvas),
                  provenOwnedBufferAdapters=sum(entry['callable'] for entry in entries.values()))
    return dict(schemaVersion=1, kind='runtime-discovery-catalog', image=pe.identity(), version=reflection.get('version'),
                counts=counts, completeEngineApi=False, types=reflection['types'], components=components['registries'],
                functions=[entries[rva] for rva in sorted(entries)],
                limitations=native['limitations'] + ['native signatures and gameplay effects are incomplete',
                    'captured heap addresses are evidence only; Lua resolves the current process dynamically'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--index', type=Path)
    args = parser.parse_args()
    result = export(PE(args.executable), args.capture)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, separators=(',', ':')) + '\n', encoding='utf-8')
    if args.index:
        args.index.parent.mkdir(parents=True, exist_ok=True)
        index = {key: result[key] for key in ('schemaVersion', 'image', 'version', 'counts', 'completeEngineApi', 'limitations')}
        index.update(kind='runtime-discovery-index', catalog=os.path.relpath(args.out, args.index.parent).replace('\\', '/'),
                     regeneration='profile-tools/dev/tools/enshrouded/export-runtime-bindings.py',
                     runtimeApi='runtime.types / runtime.values / runtime.ecs / runtime.functions',
                     productionProfile=False)
        args.index.write_text(json.dumps(index, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result['counts']))


if __name__ == '__main__':
    main()
