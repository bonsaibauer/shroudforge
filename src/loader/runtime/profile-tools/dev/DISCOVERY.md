# Runtime discovery, client and dedicated server

The discovery tools extract the complete **reflected type registry** of an EXE,
including fields, enum values, attributes, defaults, four hash domains and
metadata RVAs. They also enumerate every PE exception range, follow chained
unwind information to group procedure fragments, disassemble those ranges,
record direct calls/jumps and hash references, and inspect live metadata tables.
They do **not** claim that every engine function has a known ABI or gameplay
meaning. There is no observed universal `hash -> callable engine function` table.

The source for the reflection layout is
`src/parser/kfc-parser/crates/kfc-base/src/reflection/extract/parser.rs`.
The extractor preserves addresses which the parser normally discards, without
modifying the parser submodule. Chained unwind parsing follows Microsoft's
[x64 exception-handling format](https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64).

## Run a complete capture

From the repository root, create an isolated development environment once:

```powershell
python -m venv target/runtime-discovery-venv
$python = 'target/runtime-discovery-venv/Scripts/python.exe'
& $python -m pip install -r src/loader/runtime/profile-tools/dev/tools/enshrouded/requirements-discovery.txt
```

Build the shared production verifier, start client and server, then run:

```powershell
cmake --build build/native-runtime --config Release --target kfc-runtime-verify-component-registry
& src/loader/runtime/profile-tools/dev/tools/enshrouded/run-runtime-discovery.ps1 `
  -Python $python -VerifyBufferAdapters
```

The wrapper uses the Steam directories by default. It accepts `-ClientDirectory`,
`-ServerDirectory`, `-Target client|server|both`, `-OutputDirectory`, `-Offline`,
`-NativeVerifier`, and `-VerifyBufferAdapters`. A live capture automatically
verifies a freshly discovered manager with the production reader and writes
`runtime-bindings.json` plus `runtime-index.json`. `-VerifyBufferAdapters` also
runs the opt-in Lua corpus and isolated owned-buffer differential tests; without
that switch the export does not invent adapter validation.
A live pass reads committed readable memory and can take minutes for a large
client. It does not inject code, suspend the process, write memory, or call game
functions. Pages that cannot be read are listed; a running process is not an
atomic snapshot. Every reflected descriptor header and registry slot is checked
against the selected executable; candidate table entries are read back again.

For explicit input and a single process:

```powershell
& $python src/loader/runtime/profile-tools/dev/tools/enshrouded/discover-runtime.py `
  'C:/Program Files (x86)/Steam/steamapps/common/Enshrouded/enshrouded.exe' `
  --profile src/loader/runtime/profiles/enshrouded/client/1076226.json `
  --pid (Get-Process enshrouded).Id --functions `
  --out src/loader/runtime/profile-tools/devdata/my-client-build
```

`--profile`, `--pid`, and `--functions` are independent optional arguments. Static
reflection extraction needs only Python's standard library; live scanning uses
NumPy and function analysis uses NumPy and Capstone. No existing component map
is needed to find the reflection registry or allocation-shaped ECS table
candidates. A supplied profile supplies additional index/size checks, not names
for unknown functions. A changed executable requires a separate output directory.

## Artifacts and evidence levels

| File | Meaning |
| --- | --- |
| `reflection.json` | All entries in the discovered registry, original indices, field types/offsets, metadata addresses and hash collisions. Registry index is **not** an ECS component index. |
| `components.json` | Complete live engine registrations, separate runtime/configuration types, native callback references and independently observed entity-manager links. No component profile is used. |
| `functions.json` | Every exception range and chained procedure group; byte fingerprints, direct branches, immediate hash candidates, metadata references and data pointers into code. |
| `live.json` | Exact EXE/PID identity, regions and unreadable pages, validated metadata, contiguous pointer runs and sparse allocated table candidates. |
| `profile-audit.json` | Executable identity, hook/patch signature hits and world-operation guard checks for the supplied profile. Byte matches do not test gameplay semantics. |
| `runtime-bindings.json` | All reflected types, registrations and provisional native bindings, with separately validated adapters when requested. |
| `runtime-index.json` | Small executable/build index pointing at the full generated catalog. |
| `summary.json` | Counts and measured coverage. `fullRuntimeResolved` remains false. |
| `client-to-server.json` | Name-based layout differences and candidate counterparts of known operations across builds. |

All reports identify the executable by SHA-256, PE timestamp, image size and
target filename. `version` comes separately from the adjacent KFC data header;
it must not substitute for executable identity. Reusing a directory containing
reports from a different EXE is rejected.

Qualified hashes, internal hashes, impact hashes and short-name hashes are
different namespaces. All collisions remain explicit. Duplicate attribute
names follow the parser's last-value behavior, with displaced values retained
in `attributeDuplicates`.

`allocated-ecs-table-candidate` means a sparse array of unique reflected ECS
descriptors with an observed preceding byte-size field. Its engine role remains
a hypothesis. `profile-anchored-component-table` additionally agrees with at
least four existing component mappings and contains no observed profile
conflicts. Allocation shape does not prove every application-level bound.
Empty slots, absent types, and conflicting observations must not be invented.

Matching relocated procedure fingerprints masks only instruction address
operands, retaining other code bytes and immediate constants. All chained
fragments must match. Callee-graph candidates are weaker evidence and are listed
separately. Neither method proves the target's context lifetime, thread,
argument types, function effects, or callees' semantics. Candidates cannot be
called through Lua and are never promoted to production profiles automatically.

Exception metadata can omit leaf functions, inlined code and generated code.
Code pointer targets without exception coverage and undecodable tails are
reported. Thus a list of all exception ranges is not a list of all engine APIs.

## Look up types and compare builds

```powershell
& $python src/loader/runtime/profile-tools/dev/tools/enshrouded/inspect-runtime.py `
  lookup src/loader/runtime/profile-tools/devdata/my-client-build `
  'keen::ecs::CurrentTransform' --out target/current-transform-evidence.json

# Decimal or 0x-prefixed hash; qualifiedHash is the default domain.
& $python src/loader/runtime/profile-tools/dev/tools/enshrouded/inspect-runtime.py `
  lookup src/loader/runtime/profile-tools/devdata/my-client-build 0x1399e0ba `
  --hash-kind qualifiedHash --out target/type-evidence.json

& $python src/loader/runtime/profile-tools/dev/tools/enshrouded/inspect-runtime.py `
  compare <old-capture-directory> <new-capture-directory> --out target/build-diff.json
```

The lookup retains **all** matching types, their live table slots, and relevant
code/hash evidence. The comparison rejects reports with mixed EXE identities.
Field comparisons resolve referenced types to names instead of comparing
build-dependent reflection indices.

## Lua interface

After building/installing the updated loader and restarting the game:

```lua
local transform = game.types.get_by_qualified_name("keen::ecs::CurrentTransform")
local same_type, reason = game.types.get_by_qualified_hash(transform.qualified_hash)
assert(same_type, reason)

-- query/read/write/query_bounds accept qualified hash, name or Type.
local entities, query_reason = runtime.ecs.query(transform.qualified_hash)
if entities and entities[1] then
    local value, read_reason = runtime.ecs.read(entities[1], transform.qualified_hash)
end

-- Includes unresolved candidates, unlike get_components().
local catalog = runtime.ecs.get_catalog()
for _, entry in ipairs(catalog.entries) do
    print(entry.qualified_name, entry.qualified_hash, entry.resolved,
          entry.read_available, entry.write_available, entry.reason)
end

for operation, status in pairs(runtime.get_operations()) do
    print(operation, status.available, status.reason)
end
```

`Type` exposes `qualified_hash`, `internal_hash`, `name_hash`, and `impact_hash`.
`game.types.get_by_internal_hash(hash)` returns `nil, reason` on ambiguity.
Numeric ECS arguments always mean a **qualified type hash**, never a function
address or component index. Existing phase, capability, live layout, size,
entity generation and write checks still apply. A mapped type need not occur on
an entity; a write-capable component can still contain unsupported dynamic
fields. No API for arbitrary native calls is introduced.

## Validation

The full API contract and examples are in
[runtime-registry.md](../../../../../docs/sf/runtime-registry.md). The live runner
now follows sparse storage arrays to full engine registration records. The
[follow-up investigation](investigations/2026-10-08-component-registry.md)
explains why 81 registered components have configuration but no entity storage.

`discover-components.py EXE --capture DIRECTORY` augments a capture from a still
running process. The native `kfc-runtime-verify-component-registry` target uses
the production C++ reader in a separate read-only process to cross-check it.

```powershell
& $python -m unittest discover -s src/loader/runtime/profile-tools/dev/tools/enshrouded/tests -v
cargo test -p shroudforge-api --lib --offline
cargo build -p shroudforge-modloader --lib --release --offline
```

See [the 2026-10-08 investigation](investigations/2026-10-08-runtime-discovery.md)
for measured client/server results and the remaining gaps.

## Full runtime binding export and callback verification

The native production reader now inventories unwind groups plus code pointers in
loaded data sections. Lua merges in all live component callbacks and exposes every
candidate through `runtime.functions.list/get` with `unclear_<RVA>` names and
separate signature/context/effect validation. Code pointers can be internal labels.
The bounded proof checker can provide owned-buffer adapters without making an
unresolved native ABI callable. See [the public schema](../../../../../docs/sf/runtime-registry.md).

For each current capture, run the native verifier using its current PID and
manager address, then run the opt-in Rust corpus test and differential verifier:

```powershell
$env:SHROUDFORGE_TEST_FUNCTIONS = "$capture/provider-verification.json"
cargo test -p shroudforge-api --lib --offline production_callback_binding_corpus -- --ignored --nocapture
& $python src/loader/runtime/profile-tools/dev/tools/enshrouded/verify-buffer-adapters.py $executable `
  --registry "$capture/provider-verification.json" `
  --adapters "$capture/provider-verification.json.adapters.json" `
  --out "$capture/adapter-verification.json"
& $python src/loader/runtime/profile-tools/dev/tools/enshrouded/export-runtime-bindings.py $executable `
  --capture $capture --out "$capture/runtime-bindings.json" --index $indexPath
```

The differential verifier executes only the accepted straight-line copy bodies in
its own process with owned buffers. It never writes to a game process. Generated
full exports include all reflected types and all registrations; small indexes
point at them without checking large generated data into source control. Current
server index: `function-catalogs/enshrouded/server/1024233.json`. The legacy server
compatibility profile filename `1076226.json` does not define the observed build.
