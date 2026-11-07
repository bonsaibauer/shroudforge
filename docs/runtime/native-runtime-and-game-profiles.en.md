# Native runtime and game profiles

The native runtime is a built-in part of ShroudForge. It connects the active
parser type registry to live engine component registrations and execution-system
descriptors. Mods can inspect the resulting types, fields, storage roles, and
system dependencies. Entity reads require a validated world manager and stable
storage snapshot. Entity writes additionally require the supported engine
execution phase. Players do not install it separately.

For a player-friendly explanation of the difference between a mod's Lua code and the native KFC runtime, see the [runtime guide on the website](https://bonsaibauer.github.io/shroudforge/en/#runtime).

The [registry API](runtime-registry-api.en.md) exposes the complete reflected type
registry, owned value codecs, engine component registrations and native procedure
metadata. Component identity can now be resolved directly from the live engine
registry; typed native operations still require a verified calling contract.

This guide brings the runtime, its shared interface, and game profile workflow together. It is for contributors working on the loader.

## KFC4 live resources

`game.assets.get_loaded_resource(guid, type, part)` and
`game.assets.get_loaded_resources_by_type(type)` resolve resources from the
current process's ResourceService. Their `Resource.data` values are decoded by
the active KFC parser reflection. `Resource.source` is `loaded-runtime` and
`Resource.loaded` is true. These snapshots are read-only. Assigning to `data`
returns an error and loaded values are excluded from file asset commits.
`Resource.session_id` records the creation world session. Menu snapshots have
zero and remain readable, but cannot authorize later writes. A resource acquired
in one world cannot authorize mutations in another world.

The native reader requires an exact-image KFC4 profile, including
`enshrouded-client-25966634` and the hotfix profile `enshrouded-client-25988036`.
It validates the client owner chain,
ItemRegistry handle, ResourceService index, key, chunk and byte span on each
read. Rust supplies the canonical `ResourceId::static_hash()` from the KFC
parser, so mods do not duplicate hash calculations. The reader copies bytes
with bounded process-memory reads. It makes no engine calls, writes no game
memory and installs no hooks. The new functions are optional ABI-12 exports, so
older runtime DLLs still load and report the live resource API as unavailable.

Each call that lists resources reads the current index. A direct GUID lookup
captures its checked owned bytes immediately. Listing entries copy their payload
on first `data` access. Decoding and field wrappers are cached, so the Resource
object remains a snapshot. Request a new Resource object to obtain another snapshot.
The profile proves this path for the exact current client image. It does not
establish resource ownership across other builds or gameplay effects from
changing a value.

`game.assets.get_loaded_resource_by_field(type, field_path, value)` resolves a
unique loaded resource without each mod building its own identity index. The
path must select fixed inline storage ending in one aligned 32-bit integer,
finite float or enum. It supports reflected nesting, typedefs and static array
elements, and rejects pointer and blob-container traversal. Values use the
existing parser codec, including enum names. Float32 matching compares exact
encoded bits, so positive and negative zero are distinct.

The function returns `resource, reason, progress`. Its first call registers a
shared index job. The existing runtime loop advances that job even if the mod
does not call again or the player stops the triggering action. A later call
retrieves its result while `progress.phase` is `indexing` or `confirming`.
Discovery uses native batches of up to 64 scalar keys. These are batches per
queue step, not a requirement for 64 payload reads or repeated Lua calls.
No candidate escapes before the full captured listing is indexed.
Duplicate scalar values return an explicit ambiguity, and a missing value is
reported only after completion. `progress` includes `session_id`, `scanned`,
`total`, `payload_reads`, `complete` and `phase`. `payload_reads` counts complete
candidate snapshots read by that lookup, separately from background key batches.

`game.assets.prepare_loaded_index(type, field_path)` registers the same job
without a search value and returns `ready, reason, progress`. Call it from
`on_load` to start preparation before an action needs the result. Registration
can precede world availability. It remains associated with the active mod and
automatically restarts for a changed world session. Disabling the mod removes
its registration while retaining other mods' interest in the same index.

All admitted jobs share one cooperative 1 ms maintenance budget after the
current round's Lua callbacks, with at most 1,024 keys per round. The initial
type listing still uses one synchronous native operation and counts against
that budget. A native operation or OS scheduling can exceed the time target.
Diagnostics report actual duration and overruns. Older providers and supported
read contexts without the scalar-batch contract retain checked payload reads.

All mods in the Lua state share at most 16 indexes keyed by the captured world
session, resource type and field path, with a conservative 16 MiB shared index
admission estimate. Reaching capacity returns a reason and preserves existing
jobs. They retain ResourceIds and scalar bits, never process pointers or resource
payloads. Menu session zero remains readable. A world transition resets indexed
values while preserving declarations. Confirmed Lua resource writes invalidate
only indexes of the same type with overlapping key bytes, including writes
through template views. Changing glider pitch therefore preserves an Item-ID
index. Uncertain writes invalidate conservatively and retain the existing write
fault. Missing and ambiguous results requeue their listings after two seconds
when queried again, so later loaded resources can appear. A match
is reread, its scalar checked again, and its fresh bytes preserved in the returned
Resource. A changed or unreadable candidate discards its index. Uniqueness is
relative to the completed index snapshot, not an atomic observation of concurrent
changes to every resource. Callers must still validate the reflected resource
category they intend to use.

`game.assets.resolve_loaded_resource("g38_items::ItemRegistry", item_id)` uses
the separately validated original ItemRegistry map to select its canonical
`ItemInfo`. It returns `resource, reason`, including a fresh confirmed snapshot.
This direct relation needs no full ItemInfo scan. It currently accepts this one
original registry name and a nonzero unsigned ItemId, an active ingame Runtime
mod, the matching exact profile contract and the current nonzero local-authority
world session. An unknown relation or unavailable contract returns a reason.
It selects the original registry entry rather than proving uniqueness across
all loaded resource payloads. Its returned Resource uses the same public
writers, claims and session checks as other loaded snapshots. Flight requests
this original relation first and prepares its generic field-search fallback in
`on_load`.

The common API caches immutable reflection field plans and the attribute
catalog's name/root ordering. Current entity values, initialized roots,
world identity and write authority are checked afresh. Fixed inline resource
fields reuse these plans. BlobArray and BlobVariant traversal still checks
the actual descriptors in each resource snapshot.

`runtime.overrides.new()` registers a reversible scope with the loader.
`runtime.overrides.attribute`, `.component`, `.resource`, `.template` and
`.service` bind the common typed readers, expected-value guards, original
calculation/normalization and confirmed results. Independent mods use these
same public factories. Their packages have no dependency on Survival or Flight.

The API remembers each original baseline and holds exclusive storage claims
against other mods. An entity claim covers its original component, a resource
claim covers its ResourceId and a service claim covers its original named
binding. Several fields from the same mod may share that storage. Raw public
writers also refuse another mod's active claim. A leaked active token remains
blocked until cleanup or process restart. Unclaimed raw data writes deliberately
do not promise automatic restoration.

The loader restores registered scopes on disable, unload and callback failures,
including mods without `on_unload`. Busy cleanup keeps its writer scope and
retries while regular mod updates and UI actions are paused. `restore_disabled`
reconciles setting state. `restore_unused` restores dynamic selections omitted
since `begin_update`, such as a glider after landing. Settings are read through
the caller-bound `shroudforge.settings.get`; their UI and defaults belong in
that mod's `extended.mod.json`.

```lua
local changes = runtime.overrides.new()
return {on_update = function()
    changes:begin_update()
    changes:restore_disabled(function(key) return shroudforge.settings.get(key, false) end)
    if not shroudforge.settings.get("infiniteStamina", false) then return end
    local players = runtime.ecs.query("keen::ecs::PlayerInput")
    for _, entity in ipairs(players or {}) do
        local target = runtime.overrides.attribute(entity, "Stamina_Min")
        if target then
            changes:apply(target.key, "infiniteStamina", target, function(snapshot)
                return snapshot.values.Stamina_Max
            end)
        end
    end
end}
```

These are guarded data changes. They do not establish the gameplay semantics
of an arbitrary reflected field or authorize an unsupported native call.
Resource targets retain their creation session. Shared resource data can survive
world changes, so disable shared settings before leaving the world. A changed
world with unrestored shared ownership blocks further writes and requires a
process restart rather than treating the changed data as a new baseline.

`game.assets.read_loaded_field` decodes the scalar and exact reflected type from
an immutable loaded resource snapshot. Reading the returned snapshot after a
successful write never performs a second unlocked live observation.
`runtime.values.normalize_field` applies the same inline field codec as the
writers. `runtime.ecs.write_fields` additionally returns confirmed selected
fields as its third result, after the original attribute calculation.

`game.assets.write_loaded_field(resource, path, value, expected)` is the separate
live scalar update operation. It requires `runtime.assets.write`, the original
`runtime.ecs.write` authority, the same nonzero creation world session, Runtime
capability and an active ingame mod. The path uses the same reflection as the
parser. It supports nested structs, owned `BlobArray` indices and
`BlobVariant.value`, ending in exactly one naturally aligned 1/2/4/8-byte scalar,
including boolean flags, integers, finite floats and enums. Lua indices are
one-based. Dynamic descriptors and references cannot be written. Older providers
can reject widths other than four bytes. Without `expected`, the complete input
Resource's immutable source snapshot must still match. An explicit expected
scalar instead checks the caller's observed value. A mismatch rejects the update.

The native operation pins the exact expected session, holds its original
authoritative Scene and registered content worker lifetime gates, and verifies
the ResourceService, Reader and private bulk. It compares the complete fresh
resource snapshot before changing the selected scalar atomically, then verifies
the resulting resource and restores the original data-page protection.
It never changes executable pages. An uncertain result disables further
public data writes in that same native binding session. It provides neither
update notifications nor mutation of multiple values in one call. Success also
includes independent complete native readback under those same engine gates,
preserved as the exact returned Resource snapshot. Each input Resource keeps
its original bytes. A later unlocked observation or session change does not
turn confirmed native success into a failed write or latch a global fault.
Subsequent accesses still obey the original view and snapshot session scope.
Code following the original consumer is still needed to establish which
gameplay calculation uses a selected field. Live engine-effect verification is
separate from the structural write checks.

The [Creative Mode investigation](../evidence/runtime/2026-10-16-creative-mode.de.md)
records the owner chain and the generic ResourceService resolution proof. The
profile is marked `read-validated`, separate from an approved execution
profile. Its `ownerLayout` is shared with the ECS reader. The separately
validated `ecsReadLayout` binds the original world manager and enables the
existing typed ECS read APIs without a dispatcher hook. These sections do not
authorize native world actions. The optional `resourceWriteLayout` adds the
separately validated scalar data contract. The optional `ecsWriteLayout` adds
the local-authority Scene gate and the current component tracking policy.

The ECS reader follows the active local authority scene for offline and locally
hosted worlds, and the client replica when no local authority is active. The
native diagnostics report that choice as `entityReadRole`. Reads validate the current
scene, manager, table, entity generation and component column. A query can find
component presence without a readable payload column. Such a result does not
authorize an assumed layout or a write.

`runtime.ecs.get_template(entity)` follows the current entity's validated
definition UUID and resolves its loaded `keen::ecs::TemplateResource` through
the same ResourceService API. Mods can select component variants by their
reflected type names. No fixed player template name, GUID or component index is
needed. Whether a system consumes the template payload directly or a runtime
copy must still be established from that system's original dataflow.

`runtime.ecs.get_template_component(entity, component)` supplies a checked typed
view of one uniquely registered configuration variant. Its read and write paths
are component-relative, so mods do not walk BlobVariant arrays or copy GUIDs and
offsets. The view has `scope="shared-template"` and remains tied to its original
session and resource owner. `view:write_field(path, value, expected,
expected_fields)` requires the previous scalar. Its optional fourth argument
checks a nonempty map of at most 1,024 other component-relative expected scalars
against the fresh snapshot before the full native resource comparison. A write
derived from both fall start and lethal distance can therefore require that both
observed inputs still agree. Every entity using the template shares its changes.
Its internal registration lookup transfers only the current matching rows,
including every alias match needed to reject ambiguity. Callback disassembly
and unrelated registrations remain available through inspection APIs and are
excluded from this small lookup. Providers without the optional compact export
use the existing full registration report.

Named attribute reads can use the optional native multi-component snapshot
operation to inspect all compatible root candidates together. It amortizes
owner and entity validation while retaining initialized-header checks and
rejection of multiple matches. This does not promise an atomic snapshot of
unrelated gameplay values. The public Lua call signatures stay the same.
Providers without this checked batch export report the candidate snapshot as
unavailable instead of inferring a unique root from incomplete reads.

`runtime.services.get(original_name)` uses separately validated original named
DI service bindings. The resulting reflected plain-data view exposes the shared
authoritative scope and world session. `service:write_field(path, value,
expected, expected_fields)` uses the existing inline codec and complete native
service snapshot CAS. The optional fourth argument checks up to 1,024 nonempty
service-relative expected fields before mutation. Unsupported services and
engine-owned dynamic layouts remain unavailable. These common data contracts do
not synthesize engine events or prove a particular gameplay effect.

The World Editor now prefers live `keen::ItemInfo` resources when deriving
placement recipes. If the live API is unavailable, it falls back to the KFC
file snapshot and logs which source it used. Its world cursor, ECS, voxel and
entity actions still use the existing profile-backed runtime operations.

## How the pieces fit together

The loader is written in Rust, and the native runtime is written in C++. A shared set of rules lets them call each other. Developers call these rules an **ABI** (Application Binary Interface).

~~~mermaid
flowchart LR
    A[Active EXE and KFC] --> B[Parser types and fields]
    A --> C[Live component registrations]
    A --> D[Engine execution descriptors]
    B --> E[Runtime and template type contracts]
    C --> E
    B --> F[Resolved system dependencies]
    D --> F
    E --> G[Validated world manager and stable storage]
    G --> H[Typed ECS entity reads]
    G --> K[Verified execution phase]
    I[Reviewed native action contracts] --> J[World and lifecycle actions]
    K --> J
~~~ 

At startup, the Windows bridge finds **kfc-runtime.dll** and checks its ABI
version before calling runtime functions. If the version is not supported, the
bridge stops safely. Component registration discovery and metadata inspection
have their own readiness state. A missing game profile does not establish an
entity manager, dispatcher, or writable world context.

The runtime is built and shipped with ShroudForge. There is no separate runtime download or release.

### Asset preparation before game startup

The startup proxy installs a temporary gate at the executable entrypoint. The
main thread waits there, outside the Windows loader lock, while the bootstrap
worker prepares the asset files. It restores the original entrypoint instructions
before letting the game read those files. Runtime initialization continues on the
worker thread.

An asset-mod error restores both saved baseline files and allows the game to
start without asset patches. The failed preparation remains visible in mod
status and the log. Startup stops only if preparation fails and the baseline
cannot be restored safely. Runtime mods with export permission do not trigger
another pregame export pass or run their entrypoints during asset preparation.

`modules/entrypoints/windows-bootstrap/tests/run.ps1` verifies entrypoint ordering, DLL loading
outside loader lock, instruction restoration, and the unrecoverable-failure path.
The ignored `blueprint_startup_matches_legacy_and_recovers_from_mod_failure` Rust
test uses `SF_BLUEPRINT_GAME_SOURCE` to copy a client or server installation's
executable and asset backups into a temporary directory. It compares the current
and historical blueprint patches and verifies recovery after an intentional Lua
error, without changing the installation.

## The shared interface

The official interface is declared in [runtime_api.h](../../modules/game-runtime/native/abi/runtime_api.h). It describes the names of calls and the exact shape of the information passed between the bridge and runtime. There is no second hand-maintained list.

**KfcRuntimeAbi()** returns the interface version. If a change breaks compatibility, the version must be increased and the ShroudForge loader must be updated to match.

The interface includes startup and status calls, access to supported ECS data, profile-backed world operations, and guarded patch operations. **ECS** is how Enshrouded stores game objects and their properties.

The parser supplies component field layouts. Runtime and configuration types
can differ, so live registration descriptor pointers select the correct storage
type. Entity tables, archetype storage, callback execution phases, and native
action contracts require their own evidence. Their current implementation still
uses reviewed profile adapters. Resolving every component type does not verify
these adapters.

**KfcRuntimeStatus** returns a short health message. **KfcRuntimeDiagnostics** returns a size-limited JSON report with a schema version. The loader diagnostics module adds mod status, session controls, and measurements shown in the user interface.

## Game profiles

A **profile** is reviewed information for one Enshrouded game version and target, such as the client. The runtime uses it to identify supported game structures and decide which operations are available.

One complete profile per build lives under:

~~~text
modules/game-runtime/profiles/<game>/<target>/<build>.json
~~~

For example, the profile for Enshrouded client build **1076226** is under **modules/game-runtime/profiles/enshrouded/client/**.

Only reviewed information needed by native action, thread, and storage adapters
belongs in a release profile. Component and execution-system discovery use the
active engine metadata. Scan notes, raw captures, and unapproved values are
investigation material; keep them under **dev/profiles/captures/**.

Before approval, check the game and build identity, where the information came from, the component map, and the required observations from the live game. The launcher installs profile files under `shroudforge/runtime/profiles/`, selects the matching profile automatically, and lets the player choose another profile. Structural execution-profile revalidation stays within the detected KFC format. A KFC3 execution profile cannot authorize a KFC4 process. Verified signatures and live layout checks still decide which execution operations work.

Both native profile readers use that external catalog and the configured
runtime directory. No profile is embedded in the provider DLL. Normal exact
profiles take precedence over experimental `beta_*` profiles. The optional
[game-profile-discovery module](../../modules/game-profile-discovery/README.md)
compares exact cached baseline and current executable images before publishing
a complete beta profile in the same client or server directory. Its toggle is
on the game-profile card, which also shows the actual active profile when the
selection is automatic. Missing or ambiguous evidence leaves the candidate
unresolved. Existing live ownership and operation checks still apply.

Maintainer tools live under **dev/profiles/**. Start with the [profile development guide](../../dev/profiles/README.md).

The [runtime discovery tools](../../dev/profiles/README.md)
extract all reflected types and inspect client/server code and live tables per
executable SHA-256. Lua resolves reflected types by qualified name and can pass
those names or returned `Type` objects to ECS query/read/write operations. `runtime.ecs.get_catalog()`
reports reflected candidates and their current mapping/access status, including
unresolved components. Discovering a reflected type does not establish a native
function's calling convention or effects.

## Where the source lives

| Path | What it contains |
| --- | --- |
| **modules/game-runtime/native/abi/runtime_api.h** | The shared interface. |
| **modules/game-runtime/native/** | C++ runtime source and build files. |
| **modules/game-runtime/profiles/** | Reviewed game profiles. |
| **dev/profiles/** | Maintainer tools for examining profiles. |
| **modules/runtime-diagnostics/** | Status and measurements shown by the loader. |

The root **CMakeLists.txt** builds the runtime. **dev/build/release_build.ps1** assembles the complete Windows release.

### World session identity

`runtime.world.session_id()` returns a nonzero, process-local ECS session marker,
or `0` while the active entity table is unavailable or has not been adopted.
It requires the same runtime read access as `runtime.ecs.query`. The optional
native export is `KfcRuntimeWorldSessionId`; old providers return `0` through
the loader. This is a conservative lifetime token, not a persistent world ID:
an ECS layout rebuild also invalidates it. Retain the token with undo journals
and reject work if it is unavailable or changed; do not treat missing handles
from an invalidated session as proof that an old placement was removed.
