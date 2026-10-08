# Client/server runtime discovery — 2026-10-08

**Follow-up:** [Full component registration](2026-10-08-component-registry.md)
identifies all 598 engine registrations and explains the 81 null runtime slots
as configuration-only entries. It also documents the expanded Lua API and the
remaining native-call limitations. The measurements below describe the earlier
inventory stage.

The tools now reproduce the entire reflected type registry, retain its addresses
and hashes, join it to live table candidates, and compare native code across
builds. **The entire engine runtime is not semantically resolved.** In particular,
enumerated code and matching hashes do not provide every function's arguments,
thread requirements, object lifetime, or effects.

## Executables examined

Both installed processes were running during capture. Reads did not modify
process memory, saves, game assets or installed profiles.

| Measurement | Client | Dedicated server |
| --- | ---: | ---: |
| Adjacent KFC data build | 1076226 | 1024233 |
| PE timestamp | 1782724296 | 1778248905 |
| PE image size | 47,869,952 | 31,092,736 |
| Reflected types, all extracted and live-validated | 14,398 | 12,969 |
| Reflected struct fields | 42,729 | 32,811 |
| PE exception ranges | 69,149 | 45,282 |
| Procedure groups after following chained unwind info | 38,906 | 26,617 |
| Ranges containing matching hash immediates | 1,360 | 1,029 |
| Direct call/jump records | 300,907 | 165,775 |
| Data pointers into executable code | 20,098 | 17,399 |
| Observed component array slots | 598 | 598 |
| Non-null entries matching the existing profile | 517 | 517 |
| Null slots in that array | 81 | 81 |
| Unreadable pages during the recorded full pass | 0 | 0 |

Client SHA-256:
`af2f5a1227911d8aa06b3908d6bd0211838211cae14ea91099cb57d0df990781`.

Server SHA-256:
`001c1b40ed091d8c1aee583adde3800d7c858ae2c7f4dff54fca2938b2be1637`.

The server profile is named `server/1076226.json` and has ID
`enshrouded-server-1076226`, but its SHA-256 correctly identifies this server EXE.
The adjacent server KFC data header reports **1024233**, also matching its parser
cache version. File/profile labels and KFC data versions must not substitute for
the executable fingerprint. No production profile was renamed or replaced.

## Confirmed observations

- The independent extractor agrees with the KFC parser caches on all 14,398
  client and 12,969 server type records checked: names, all four hashes, primitive
  kind, size/alignment, field count, field names, field type indices and offsets.
- Each live descriptor header and static reflection registry pointer matched
  the selected executable. Table entries were reread after scanning.
- Large static reflection tables use a different order from runtime component
  indices. Treating their position as an ECS ID would be incorrect.
- The sparse runtime table candidate has a preceding allocation byte length of
  4,784 (= 598 pointer slots). Its 517 non-null entries agree with known component
  names, indices and sizes, with no observed conflicts. The server table was also
  rediscovered in a separate pass with **no profile input**. Its array shape alone
  is not proof of an arbitrary new build's engine semantics.
- No qualified-hash collisions occur in these registries. Internal hashes have
  6,232 client and 5,563 server collision buckets; choosing the first such match
  is incorrect. The new Lua lookup reports ambiguity explicitly.
- Comparing reflected layouts by qualified names gives 12,957 unchanged types,
  12 changed layouts and 1,429 client-only types. The changed types are six
  resource/UI types and their `keen::ds` counterparts. No ECS field-layout changes
  were found among types shared by these two registries.

## Concrete server function counterparts

The existing server profile's six World function guards fail against this EXE.
Its configured global world RVA is also outside the server image. Several hook
signatures and all six patch signatures do match; this does not make the other
World operations available.

Five source operations have exactly one server procedure group with identical
code after masking instruction address operands. Every chained fragment is
included, rather than only the first range/prologue:

| Operation | Client RVA | Server candidate RVA | Compared code bytes |
| --- | --- | --- | ---: |
| `runtime.world.entity.spawn` | `0x3e1120` | `0x1bc6a0` | 838 |
| `runtime.world.entity.place` | `0x3ebb70` | `0x1c71c0` | 309 |
| `runtime.world.entity.destroy` | `0x3ebcb0` | `0x1c7300` | 293 |
| `runtime.world.voxel.read` | `0xe819d0` | `0x7179f0` | 1,318 |
| `runtime.world.voxel.write` | `0xe8cb20` | `0x722d20` | 621 |

These are **code counterparts, not tested callable server bindings**. Call/RIP
targets are masked, so their referenced objects and callees still need semantic
validation. `finish_building` has no identical group; callee-graph comparison
suggests server RVA 1,846,720 as its first investigation lead (528 bytes versus
450 client bytes). That lead is weaker evidence and remains unverified.

A Capstone 5.0.7 encoding-size issue was encountered for prefixed SSE
instructions such as `movdqa`: RIP-relative `disp32` was reported as two bytes.
Normalization now verifies and masks all four encoded displacement bytes;
regression coverage includes this case.

## Lua and tools delivered

- `game.types.get_by_qualified_hash` and `get_by_internal_hash`, with explicit
  missing/ambiguous results; all four hash values exposed on `Type`.
- Qualified integer hashes accepted by existing ECS query, bounds-query, read
  and write paths, with existing provider and entity checks.
- `runtime.ecs.get_catalog()` includes unresolved reflected component candidates
  and current access status; `runtime.get_operations()` lists every operation in
  the canonical runtime operation catalog with phase/mod/provider availability.
- EXE/live discovery, hash/name evidence queries, chained function grouping and
  cross-build layout/code/callee-graph comparison, with a client/server runner.
- Python regression tests and Rust hash-lookup tests; optimized loader DLL build.

Run instructions and evidence definitions: [DISCOVERY.md](../DISCOVERY.md).
Large local evidence remains under
`src/loader/runtime/profile-tools/devdata/discovery-20261008/`:
`client/`, `server/`, `server-without-profile/`, `client-to-server.json`, and
`current-transform.json`. Captures are intentionally outside source control.

## What still prevents “all runtime functions available”

The five server counterparts have not been called to prove behavior. The server
world context chain, placement/execution contexts, Finish-Building semantics and
event ID still need validation. The 81 null component slots were not populated
by inventing mappings. There is no direct native-reference proof for the many
functions that only contain numerically matching type hashes.

Exception tables omit some leaf/inlined/generated code. The inventories record
undecodable tails (624 client ranges, 342 server ranges) and code pointers outside
exception coverage. None of those gaps is described as a complete function ABI.

The new Lua code is compiled in `target/release/shroudforge_modloader.dll`
(packaged name `shroudforge-runtime.dll`). The currently running games retain
their already-loaded DLLs. Installing the updated loader and restarting is
required to use the new Lua entry points; their gameplay behavior was not tested
inside those older running DLLs.
