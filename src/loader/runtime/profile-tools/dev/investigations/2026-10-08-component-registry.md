# Full component registration and Lua registry API

This investigation supersedes the interpretation of the 81 null storage slots
in the [earlier inventory](2026-10-08-runtime-discovery.md). It does not supersede
the finding that the engine's complete native calling ABI remains unresolved.

## Evidence from both running targets

| Measurement | Client | Server |
| --- | ---: | ---: |
| KFC data build | 1076226 | 1024233 |
| Complete engine registration records | 598 | 598 |
| Registrations with entity storage | 517 | 517 |
| Registrations with configuration metadata | 141 | 141 |
| Configuration-only registrations | 81 | 81 |
| Separate runtime and configuration layouts | 60 | 60 |
| Code pointers in registration records and parallel callback slots | 50 | 48 |
| Managers independently connected to registration owner | 2 | 1 |
| Procedure groups read by production C++ inventory | 38,906 | 26,617 |

The scan used no component profile. It followed pointers to the previously
observed sparse descriptor array, found its owner, and then found the owner's
complete registration vector. Every registration was checked against reflection
and the parallel descriptor/size arrays. A second pointer pass identified entity
managers referring to the same owner. Sampled entity archetype bits/strides
provided independent evidence connecting registration indices to component data.
Only declared registration indices were inspected; reading a fixed 1024-bit
range had included unrelated layout data beyond this build's component range.

The native provider uses **the same C++ reader** as
`kfc-runtime-verify-component-registry`. That reader was executed externally,
read-only, against both processes and validated all 598 registrations each.
It did not inject a DLL, execute a callback, or change game memory.

## Observed structure, layout version 1

The entity manager's first pointer refers to the registration owner. Offsets
below are relative to that owner, not absolute addresses or RVAs.

| Offset | Meaning established by cross-checks |
| ---: | --- |
| 8 | Pointer to 256-byte registration records |
| 16 / 24 | Record count / capacity |
| 232 / 240 | Pointer / count of 16-bit runtime sizes |
| 256 / 264 | Pointer / count of runtime reflection descriptors |
| 280 / 288 | Pointer / count of 40-byte callback-slot records |

Within each 256-byte registration:

| Offset | Meaning |
| ---: | --- |
| 0 / 8 | Short registration name span |
| 16 / 24 | Qualified registration name span, including trailing zero |
| 32 | Qualified registration hash |
| 40 | Runtime reflection descriptor, or null |
| 48 | Configuration reflection descriptor, or null |
| 56 | **16-bit** runtime size |
| 58 | Packed flags, semantics not inferred |
| 60 | Additional flags, semantics not inferred |
| 72 / 80 | Runtime default bytes pointer / size |
| 104 / 128 | Observed code-pointer slots; calling conventions remain unknown |

Treating the size at offset 56 as a 32-bit integer fails for
`NetworkAnimationGraphInput`: its 512-byte size is followed by packed flags.
Both the implementation and regression fixture retain these separately.

Example: registration `keen::ecs::ActiveNpcState` has hash `0x1a8b5909`, runtime
layout `keen::ecs::DynamicActiveNpcState` (136 bytes), and configuration layout
`keen::ecs::ActiveNpcState` (456 bytes). Registration `ActorRotation` has a
96-byte configuration layout, null runtime descriptor and runtime size zero.
The earlier profile assigned a size to that null storage entry; it must not be
treated as an actual entity column.

The provider now publishes engine-derived storage mappings, removes template-only
entries from the usable entity map, resolves original registration names to
their runtime layouts, and retains the template metadata in the Lua registry.
Unique stride/size matches remain diagnostic observations and are no longer
promoted to component identities. Once a live registry has been validated, a
subsequent validation failure clears its mappings instead of silently reverting
to profile entries.

## Lua and code exposure

The public schema is documented in [runtime-registry.md](../../../../../../docs/sf/runtime-registry.md).
The four namespaces are `runtime.types`, `runtime.values`, `runtime.ecs`, and
`runtime.functions`.

The procedure inventory is generated in the native provider from its loaded PE
exception directory. All group starts, fragment memberships and code-byte totals
were compared against the independent Python disassembler inventory: zero
differences across both targets. The inventory identifies unknown ABIs explicitly
and never fabricates a native calling signature. The subsequent provisional-binding
layer below merges registration callbacks and image code pointers as well.

The real-cache Rust smoke test verified hash resolution, inheritance traversal,
effective fields, Lua metadata access and generated struct definitions for all
14,398 client types and all 12,969 server types, including DS metadata. Owned KFC
value decoding/encoding and inheritance were additionally exercised in Lua with
an independent fixture. Native DS allocation/ownership remains unsupported.

## Reproduction

`run-runtime-discovery.ps1` now includes complete registration discovery whenever
it captures a running process. To augment a fresh existing capture:

```powershell
& $python src/loader/runtime/profile-tools/dev/tools/enshrouded/discover-components.py `
  'C:\Program Files (x86)\Steam\steamapps\common\EnshroudedServer\enshrouded_server.exe' `
  --capture src/loader/runtime/profile-tools/devdata/registry-complete-20261008/server

cmake --build build/native-runtime --config Release --target kfc-runtime-verify-component-registry
# Substitute a CURRENT PID and manager address from components.json, never a saved address after restart.
& build/native-runtime/tools/kfc-runtime-verify-component-registry.exe $processId $managerHex $outputJson
```

Current artifacts are under `profile-tools/devdata/registry-complete-20261008/`:
`client/components.json`, `server/components.json`, each target's
`provider-verification.json`, and `provider-verification.json.functions.json`.
The full discovery reports carry executable SHA-256; the verification-only
function snapshots are used for implementation comparisons and are not portable
profiles. Heap addresses in evidence are never installed as runtime addresses.

## Remaining boundary

All 598 registrations in these two live registries are accounted for. This is
not a claim that every engine type is an ECS component, that every component is
present on a live entity, or that the 598 records include all engine subsystems.
Hooks and manager/entity structure compatibility still use build evidence.
Unknown native function signatures, effects, server world contexts, live
configuration replacement and engine-managed DS allocation are not resolved by
this work. The entire engine is not yet a callable Lua API.

## Provisional bindings and remaining code references

The production inventory now also examines aligned pointers in non-executable
sections of the loaded image. Pointers are evidence of a code address, not proof
of an independent function boundary; descriptors retain that distinction. The
Lua catalog merges those targets with chained unwind groups and live registration
callbacks, including callbacks without unwind information.

| Evidence | Client | Server |
| --- | ---: | ---: |
| Chained unwind procedure groups | 38,906 | 26,617 |
| Distinct image code-pointer targets | 18,426 | 16,146 |
| Combined image code candidates | 52,144 | 38,555 |
| Distinct registered callback targets | 49 | 48 |
| Total combined Lua code candidates | 52,150 | 38,565 |
| Proven registered owned-buffer adapters | 2 | 2 |

Every entry is available as `unclear_<eight-hex-digit-RVA>` and as an
executable-SHA256-qualified key. `runtime.functions.get` returns incomplete
bindings with signature/context/effect evidence; their `call` returns a precise
reason when no executable adapter exists. Descriptor mutation does not alter
its captured implementation. Existing validated game operations retain their
checked wrappers. No raw arbitrary native-call interface was added.

The bounded x64 proof checker recognizes complete straight-line two-buffer copy
bodies. It rejects calls, branches, globals, stack use, unsupported instructions,
truncated bodies, and unproved pointer or byte-width flows. It constructs an
owned-byte interpreter, preserving destination bytes outside proved writes.
Minimum source/destination sizes are derived from all accesses, not type names.
A function is never selected by a hard-coded hash/RVA whitelist.

The Extinguish pair was accepted independently on each target. The adapter
contracts are byte transformations, not a claim about networking/gameplay
semantics. Code was checked against the executable, separately decoded with
Capstone, copied into private executable memory in a test process, and compared
with the interpreter for 256 randomized cases per body. All 1,024 comparisons
passed, including untouched destination guards, source immutability and the
constant return where known. No game process memory was written or game
operation dispatched. See `verify-buffer-adapters.py` and each capture's
`adapter-verification.json`.

The expanded native reader still exactly matches the independent Python
implementation for every original unwind root, fragment membership and code-byte
total. Live and file image pointer sets differ because writable data changes
and scanning numeric values can also produce code-address candidates; these
sets are not labeled as verified methods or signatures.

Validation after the extension: 14 Python tests; 11 normal Rust tests; explicit
real-registry tests for all client/server types; explicit production-callback
Lua tests for both targets; C++ production-reader validation against both live
processes, including three injected local-copy corruption cases; release builds
of both runtime DLLs. The ignored Rust tests are opt-in real-data tests and were
run explicitly, not assumed to pass.

## Complete local exports

`export-runtime-bindings.py` combines the full reflection metadata, all component
records, every observed code candidate, callback ownership and proven buffer
plans into `runtime-bindings.json` per target. Generated exports remain in
`profile-tools/devdata/registry-complete-20261008/{client,server}/`; small indexes
are committed under `dev/function-catalogs/enshrouded/`. The server index is
`server/1024233.json`, based on the actual adjacent KFC data version and executable
SHA256, independently of the legacy compatibility profile's filename.

These exports and provisional bindings expose the unresolved work instead of
hiding it. Unknown native parameters/contexts, unreferenced or inlined code,
engine allocation and gameplay effects remain open; a complete callable engine
API has not been established.

The complete automatic `run-runtime-discovery.ps1 -Target server
-VerifyBufferAdapters` pipeline was also run successfully against PID 7588. It
redetected all 12,969 types and 598 registrations, selected its current manager,
ran the shared C++ reader and Lua corpus, passed the native-buffer differential
checks, and exported 38,565 provisional bindings under
`devdata/pipeline-20261008/server/`. No saved manager address was supplied.

Final API review added `runtime.ecs.read_bytes` for complete owned snapshots of
partially decoded component layouts. Whole-value writes of nested unsupported DS
containers now return a reason before entering upstream unimplemented codecs;
metadata exposes `partial_value` and `value_reason`. Readable POD fields and all
raw component bytes remain accessible. This is not engine-container allocation.
