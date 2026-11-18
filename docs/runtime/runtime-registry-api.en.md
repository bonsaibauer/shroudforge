# Runtime registry API

The Lua interface has four layers. Every layer uses the current process's type
registry. Client and dedicated server expose the same API shape.

| Namespace | Contract |
| --- | --- |
| `runtime.types` | All reflected types, inheritance, fields, enums, attributes, flags and default bytes. This is the same object as `game.types`. |
| `runtime.values` | Decode, validate and encode owned KFC values using those layouts. |
| `runtime.ecs` | Engine registrations, their runtime/template layouts, and checked access to entity storage. |
| `runtime.services` | Session-bound reflected views of original named engine services with separately validated plain-data bindings. |
| `runtime.overrides` | Shared ownership, conflict detection and restoration bookkeeping for reversible Lua settings. |
| `runtime.functions` | Typed operations and build-scoped provisional bindings for all evidenced native code candidates. |
| `game.assets` | KFC file resources, live ResourceService snapshots and separately gated scalar updates on the matching KFC4 client build. |

`runtime.schema_version` is `1`. Type discovery and engine component identity
do not require a per-build component hash list. Hooks, entity layouts, world
contexts and approved native calls still require compatible build evidence.
**This is not a fully reverse-engineered engine API.** Unknown function
parameters, side effects and DS allocation contracts remain unresolved.

## Type identity

```lua
local ty = assert(runtime.types.resolve("keen::ecs::CurrentTransform"))
assert(runtime.types.resolve(ty) == ty)

for _, field in ipairs(ty.fields) do
    print(field.declaring_type.qualified_name, field.name,
          field.type.qualified_name, field.data_offset)
end
```

The supported selectors for type/value/ECS APIs are a qualified name or a
`Type` returned by the current process registry. This keeps build-specific
identity details out of mods while preserving exact type ownership checks.

On KFC4 client build 25966634, the existing `runtime.ecs.query`, `resolve`,
`read`, `read_bytes` and `read_fields` APIs can use the original engine owner
chain independently of the game-thread dispatcher. `ownerLayout` and
`ecsReadLayout` in the exact-image read profile describe this shared ownership
and storage layout. Component names, indices, runtime/template roles and sizes
come from the live engine registration, checked against parser reflection.
They are not copied into a second component profile.

In an offline or locally hosted world, this reader selects the active local
authority scene. On a remote server connection it reads the client replica.
`entityReadRole` in the native diagnostics identifies the selected realm. Server-only
components such as `Health` and `Stamina` must not be inferred from a client
replica's network components.

Queries check the original archetype's component-presence bits. Presence alone
does not prove that a component has readable storage. A read additionally
requires a valid column whose stride exactly matches the reflected storage
size. A zero or mismatched stride is reported as unavailable, including for
types with reflected fields. Queries are incremental. Retry a pending query
from a later update until it returns a result table.

This backend revalidates the owner, entity table, table membership, generation,
archetype and component bytes. A changed or unstable snapshot returns a reason
and must be retried. It installs no code hooks and makes no engine calls.
`entity_access_ready` can be true while `entity_writes_ready` is false. Writes
and world actions retain their separate execution requirements.

## Live ResourceService reads

In the ingame phase, `game.assets.get_loaded_resource(guid, type, part)` reads
one resource and `game.assets.get_loaded_resources_by_type(type)` lists live
resources by reflected type. The KFC4 profiles enable these methods only for
their exact Enshrouded client images, including
`modules/game-runtime/profiles/enshrouded/client/25966634.json` and `25988036.json`. Other builds keep the
same API shape but report that the live ResourceService is unavailable.

```lua
local items = game.assets.get_loaded_resources_by_type("keen::ItemInfo")
for _, item in ipairs(items) do
    if item.loaded then
        print(item.guid, item.source, item.data.debugName)
    end
end
```

`Resource.data` is decoded with the active KFC reflection. A direct GUID lookup
captures the complete checked snapshot during the lookup. Type listings retain
lazy payload reads on first `data` access. Its `source` is
`loaded-runtime`, and `loaded` is true. Assigning `data` is rejected. Existing
`get_resource` and `get_resources_by_type` continue to use the parser's KFC file
snapshot and report `source="kfc-file"`.
Newly created or added resources report `source="asset-transaction"`.
Loaded resources also expose their creation `session_id`. It is zero for a
menu snapshot and absent for file/transaction resources. A snapshot created in
one world cannot authorize a write in another. Menu snapshots remain readable,
but writing requires a fresh resource acquired in the active world.

Keep a returned resource object while reusing its snapshot. Its first `data`
access decodes the captured bytes, copying a payload first only for lazy listing
entries. The mapped value and complex field wrappers are cached. Request a new
resource object to read a new snapshot. A GUID lookup uses
the original indexed lookup directly. Listings reuse memory-region observations
only within that call and still exclude each unreadable resource span.

`game.assets.get_loaded_resource_by_field(type, field_path, value)` registers a
shared background index of a fixed inline aligned 32-bit scalar. Integers,
finite Float32 values, enums, nested structs, typedefs and static array indices
use the existing reflected codec. Dynamic container traversal is excluded from
this fixed-key index. A result requires the complete captured listing and a
fresh full snapshot confirming the candidate. Missing and duplicate keys return
`nil, reason`. Float32 matching preserves its exact bits, including signed zero.

The result is `resource, reason, progress`. `progress.phase` is `indexing`,
`confirming` or `complete`. Its `payload_reads` counts complete confirmation
snapshots for this lookup. The runtime advances discovery independently of
further Lua calls or a player's active action. It uses up to 64 projected keys
per native batch, with checked payload fallback when the optional optimized
contract is unavailable. All jobs share a cooperative 1 ms budget and at most
1,024 keys per runtime round. Initial synchronous listing and native overruns
are measured as part of that work.

Use `game.assets.prepare_loaded_index(type, field_path)` in an ingame mod's
`on_load` to register the same preparation before a gameplay action needs it.
It returns `ready, reason, progress`. A missing world owner leaves preparation
pending. Declarations survive world changes and are removed with their mod.
Identical type/path requests from different mods share one job. Capacity is
limited to 16 indexes and a conservative 16 MiB shared index admission estimate.
New requests exceeding capacity return a reason without evicting active jobs.

Confirmed writes reset only indexes whose type and scalar key range overlap
the written bytes. Unrelated settings preserve the index. Uncertain writes and
changed sessions reset conservatively. Read failures discard partial values and
retry with backoff, so an unreadable late candidate cannot establish uniqueness.
The completed index represents its captured listing, not an atomic global
observation of concurrent resource changes. A missing or ambiguous lookup
requeues preparation after two seconds when requested again.

For an original engine relation, use
`game.assets.resolve_loaded_resource("g38_items::ItemRegistry", item_id)`.
The current implementation resolves this validated map directly, without a
full ItemInfo scan, and checks its selected resource against the ResourceService
and active reflection. It requires a nonzero unsigned ItemId, active Runtime
mod, exact supported provider/profile and current nonzero local-authority world.
Unknown registry names return a reason. It selects the canonical original entry
and returns a normal immutable Resource snapshot. It does not provide arbitrary
native registry access or global field uniqueness. Further writes use the
existing common typed writers, ownership claims and session checks.

`game.assets.write_loaded_field(resource, path, value, expected)` requires both
`runtime.assets.write` and the original `runtime.ecs.write` authority, an active
runtime mod and the exact same nonzero world session as `resource.session_id`.
It uses the session-bound native resource CAS exclusively. Without `expected`,
the immutable source scalar of the passed Resource must still match the fresh
snapshot. An explicit expected scalar expresses the caller's current intent.
The native gate compares the complete fresh resource snapshot before changing
that scalar, so a changed BlobVariant selection, descriptor or sibling cannot
silently reuse the earlier resolved byte offset.
Only one naturally aligned reflected 1/2/4/8-byte scalar is writable, including
boolean flags and 64-bit `Time.value` fields. Older providers can reject widths
other than four bytes. The existing field walker
rejects references, container descriptors and non-finite floating-point values.
Native resource status lists supported byte widths in `resourceWriteWidths`.
The original `resourceWriteWidth=4` field remains as the preferred legacy width.
Success includes independent complete native readback under the original Scene
and ResourceService lifetime gates. Those exact confirmed bytes form the returned
Resource snapshot. Existing snapshots keep their original bytes.

A genuinely uncertain native outcome blocks further public Lua data writes in
that original session. Native success already includes complete payload and
owner validation under both engine gates. A later unlocked observation can
change without turning that confirmed write into a failure or setting a fault.
Session changes invalidate subsequent writes through old views and resources.
Stop automatic retries and restoration after a genuinely uncertain native
outcome. Neither scalar success nor readback establishes a new engine event or
the selected field's gameplay effect.

The native reader resolves the original ResourceService map, handle and chunk
without engine calls, game-memory writes or hooks. Rust passes the parser's
canonical `ResourceId::static_hash()` into the native lookup, so mods do not
need per-resource hashes. This read path does not provide live updates or
gameplay writes. Resource and ECS readers share the same original owner-chain
resolver. World operations and entity writes retain their execution contracts.

`runtime.types.get_all()` includes DS types. `get_all(false)` preserves the old
non-DS enumeration. `find({prefix="keen::ecs::", attribute="server_only"})`
filters metadata without implying that a type is an entity component.
`get_by_index(n)` accepts a zero-based reflection index for this registry only.

`Type.struct_fields` contains declared fields; `Type.fields` contains effective
inherited fields and their declaring types. `inner_type` can be an element type
or a base class; `Type:is_a(parent)` follows struct/typedef inheritance only.
`flags_bits` preserves the raw flags and `flags` exposes their names.
`default_bytes` preserves the exact optional byte string, including zero bytes.
Enum `value_hex` preserves all 64 bits without a signed-number ambiguity.
Generated Lua definitions include DS layouts; DS containers are marked as native
storage descriptions, not automatically writable Lua-owned values.

## Component registration is not the storage layout

```lua
local component, reason = runtime.ecs.get_component("keen::ecs::ActiveNpcState")
if not component then return end -- provider/world may still be starting

-- On the examined builds: registration ActiveNpcState, storage DynamicActiveNpcState.
print(component.index, component.qualified_name)
print(component.runtime_type and component.runtime_type.qualified_name)
print(component.template_type and component.template_type.qualified_name)

local registry = assert(runtime.ecs.get_registry())
for _, entry in ipairs(registry.entries) do
    print(entry.qualified_name, entry.storage, entry.runtime_size)
end
```

The older KFC3 snapshots examined on 2026-10-09 have **598 registrations** in
each executable. Of these,
**517 have entity storage**, **81 have template configuration only**, and **60
have both storage and a separate configuration type**. The total configuration
layout count is 141. A null storage pointer in the 81 entries is accompanied by
a valid configuration type and storage size zero. It is not an unresolved
component to which a size or address should be assigned.

The provider reads the registration owner, checks the record array and bounded
length, then validates every name/hash/type/size against reflection and both
parallel storage arrays. It rereads registration identity
before publishing. This resolves original names and `Dynamic*` storage names
without a component profile. The structural reader currently understands layout
version 1. Unsupported layouts fail validation; matching sizes alone never
create a mapping. Existing exact-build component profiles remain a fallback
until an engine registry has been validated.

`query`, `query_bounds`, `read`, `write`, `read_fields`, and `write_fields` accept
the original registration name or its returned `Type` and select its actual
runtime layout. A template-only registration has no entity column, so it cannot
be read or written through entity storage calls.
Its configuration uses the separate shared-template view below. This does not
implement template replacement or engine allocation.

### Typed shared-template configuration

`runtime.ecs.get_template_component(entity, component)` resolves the current
entity's original template UUID, the exact loaded `TemplateResource` and one
concrete configuration variant. The selector is the original engine registration
or its concrete template `Type`. A missing template role, a mismatched reflected
layout and multiple matching registrations or variants return `nil, reason`.
Variant selection uses the production KFC mapper. Mods do not walk variant arrays
or carry resource GUIDs, component hashes, array positions or payload offsets.

```lua
local fall, reason = runtime.ecs.get_template_component(entity, "keen::ecs::FallDamage")
if not fall then return end
local fields, reason = fall:read_fields({"fallDamageStartDistance", "fallDamageLethalDistance"})
if not fields then return end
-- Save the original start value and this view before changing shared configuration.
local ok, reason = fall:write_field("fallDamageStartDistance",
    fields.fallDamageLethalDistance, fields.fallDamageStartDistance,
    {fallDamageLethalDistance=fields.fallDamageLethalDistance})
```

The view exposes `key`, `type`, `scope="shared-template"` and `session_id`.
`type` is the canonical concrete configuration `Type`, which can differ from
the entity's runtime storage type. The key contains the resource GUID, resource
type, part and concrete component name. Mods use the key **together with the
session** to deduplicate ownership when several players share one template.
**Every entity using that template shares the changed configuration.** This is
not an entity-private setting. Engine consumption must still be established for
the selected field and each intended effect.

`read_fields(paths)` reads a fresh resource snapshot and returns values keyed by
the selected component-relative paths. It accepts a nonempty dense array of at
most 1,024 distinct paths. This initial view supports the existing ResourceService
contract of aligned 32-bit integer, float and enum scalars. Unsupported pointers,
references and container descriptors are rejected. `write_field(path, value,
expected, expected_fields)` requires the expected previous scalar, `runtime.assets.write` and
the existing `runtime.ecs.write` authority gate. It checks the fresh scalar,
uses the native atomic resource comparison, and verifies independent complete
native readback before returning success. The native comparison checks all bytes
of the fresh resource snapshot under the original Scene and ResourceService
lifetime gates, changes only the selected 32-bit scalar, then independently
reads back the complete resulting payload under those same gates.
Every access checks the exact registered configuration
variant and original world session. A view can restore
its original resource after the creating entity disappears in the same session.
It cannot follow a later player or world to a different template owner.

The optional compact native registration lookup returns every current row
matching the requested registration or concrete configuration type. The same
identity and ambiguity checks apply. This path transfers only relevant metadata
instead of parsing the full registry and callback code evidence for each scalar
read. Older providers use the existing full report.

The optional fourth argument `expected_fields` is a nonempty map of at most
1,024 component-relative field paths and expected values. It uses the same
scalar codec as reads and writes. Every expectation must match the fresh
snapshot before the native full-snapshot CAS. For the example above, both the
observed start distance and the lethal distance used to calculate the new value
must still agree. A different lethal distance rejects the operation before any
write. This expresses caller dependencies without mod-specific rules in the API.

An error containing `uncertain` requires stopping further writes and restoration
attempts through this view. A genuinely unconfirmed native mutation latches the
shared session fault. Confirmed native success returns success directly. A later
unlocked observation or world change does not rewrite its outcome or latch a
fault. Future operations still check the original session. Ordinary conflicts
and temporarily unavailable gates return a reason without blind retries inside
the API.

The lower-level parser value contract stays `Variant.type: string` and
`Variant.value: T`. This differs from `Resource.type` and the view's `type`,
which are reflected `Type` objects. The typed view hides that container detail
from gameplay mods and preserves existing `get_template` and entity field APIs.

```lua
local entities, reason = runtime.ecs.query("keen::ecs::CurrentTransform")
if entities and entities[1] then
    local value = assert(runtime.ecs.read(entities[1], "keen::ecs::CurrentTransform"))
    -- Change reflected fields, then submit this exact read-derived value:
    -- local ok, error = runtime.ecs.write(entities[1], "keen::ecs::CurrentTransform", value)
end
```

Availability still depends on the mod's runtime capability, lifecycle, thread
dispatcher, live entity generation and matching field layout. A writable entry
does not guarantee that an arbitrary change has the intended gameplay effect.
Pointer-bearing/dynamic fields are not made writable by merely naming their type.
Entries with unsupported nested containers expose `partial_value=true` and a
`value_reason`; their whole-value `write_available` is false and writes return a
reason before entering the upstream codec. POD fields can still be inspected.
`runtime.ecs.read_bytes(entity, type)` returns the complete owned storage snapshot,
including opaque container descriptors, without following any pointer. Use the
reflected field offset/size to decode a supported field from that byte string:

```lua
local bytes = assert(runtime.ecs.read_bytes(entity, component.runtime_type))
for _, field in ipairs(component.runtime_type.fields) do
    local start = field.data_offset + 1
    local value, reason = runtime.values.decode(field.type,
        bytes:sub(start, start + field.type.size - 1))
    -- Unsupported container fields return nil,reason; their bytes and metadata remain available.
end
```

### Catalog identity and current entity access

`runtime.ecs.get_catalog()` returns all reflected `keen::ecs::*` types and joins
their exact runtime/template roles from the validated engine registry. Helpers,
enums and other reflected types without a registration remain visible as
`registration_state="reflected-only"` and `storage="unregistered"`. They are not
missing component mappings that should be assigned an invented entity column.

The catalog header contains `version`, `registry_available`,
`registered_type_count`, `runtime_type_count`, `template_type_count`,
`resolved_count`, `reflected_only_count`, `entity_access_ready`,
`entity_writes_ready`, and `entries`. Type counts describe the reflected ECS
namespace, not the number of engine registration records. Runtime and template
roles can overlap, so their counts must not be added to obtain a registration
total. `entity_access_ready` and `entity_writes_ready` describe the native
provider. Each entry's availability also checks the current mod's API contract,
runtime capability and active lifecycle.

Each entry includes `type`, `qualified_name`, `qualified_hash`, `size`,
`registered`, `runtime_resolved`, `resolved`, `registration_state`, `storage`,
and `registrations`. Every relation in `registrations` contains `role`
(`runtime` or `template`), engine registration `index` and its original
`qualified_name`. The index is distinct from the parser's type index.
`storage` is `entity`, `template-only`, or `unregistered`.

`resolved` means that a runtime mapping or matching template registration has
been validated. It remains independent of the dispatcher and current world
phase. `runtime_resolved` specifically identifies matching entity storage.
A template-only type can be resolved while all entity operations remain
unavailable. An existing verified provider fallback can resolve entity storage
before the engine registry is available. In that case no observed registry role
is invented, and `registered` can still be false.

The remaining entry fields are `read_available`, `write_available`,
`read_fields_available`, `write_fields_available`, `partial_value`,
`value_reason`, `read_reason`, `write_reason`, and `reason`. A partial component
can expose `write_fields_available=true` while `write_available=false`.
Field availability grants access to the checked selection API. Each requested
path still has to resolve to supported inline storage, and an entity must
actually contain that component. A resolved mapping by itself does not make
entity reads or writes available in the main menu.

### Selected inline fields in partial components

`read_fields` and `write_fields` use reflected field names, inherited fields,
dot-separated nesting and **one-based static array indices**. For example,
`dataStorage[1]` selects the first reflected static element. Paths are resolved
against the actual runtime storage type selected for the registration.

```lua
-- entity is a live generation-checked handle from runtime.ecs.query.
local selected, reason = runtime.ecs.read_fields(entity, component.runtime_type,
    {"dataStorage[1]", "dataStorage[2]"})
if selected then
    print(selected["dataStorage[1]"], selected["dataStorage[2]"])
end

-- Explicit field-path/value map. The current component layout must contain this path.
-- local ok, reason = runtime.ecs.write_fields(entity, component.runtime_type,
--     {["dataStorage[1]"] = desired_value})
```

`read_fields` accepts a nonempty dense array of distinct paths and returns a
table keyed by those exact paths. `write_fields` accepts a nonempty
field-path/value map and returns `ok, reason`. Each call accepts at most 1,024
paths, with at most 1,024 bytes and 64 field/index steps per path. The selected
field can be a scalar, enum, GUID, inline struct or static array whose leaves
are all supported inline values. Unsupported DS, Blob and ObjectReference
descriptors are rejected. Paths never follow their pointers or resolve their
elements. A scalar sibling remains usable when another field in the component
contains such a descriptor.

Every selected field is decoded or validated by its reflected KFC type.
The API reuses bounded immutable inline field plans for the active reflection.
Their offsets and codec rules are cached. Live component values, generation,
column identity and write permission are validated at each access. Loaded
resource paths through BlobArray or BlobVariant still inspect current snapshot
descriptors and spans.
Writes reject overlapping selections, overlap with any enclosing reflected
external-storage/reference descriptor, and non-finite float values. The mask
covers only reflected value leaves. Struct padding, unselected sibling fields
and descriptors remain byte-for-byte unchanged.

`write_fields` first reads a fresh complete component snapshot inside the call.
If the requested values differ, the existing native `compare_exchange` checks
the **entire component snapshot** again inside the approved engine execution
scope before applying the
masked write. A changed snapshot rejects the write and requires a fresh call.
This is not a comparison against a previous `read_fields` result. Values that
already agree return success without a write.

These APIs still require an active ingame mod with the `runtime` capability,
the existing `runtime.ecs.read` or `runtime.ecs.write` API availability, a ready
native write scope, matching component storage and a valid entity generation.
For actual reflected `keen::ecs::AttributeComponent` subtypes, `write_fields`
and whole-component `write` also apply the shared attribute contract. Root ID,
definition identity, header and storage descriptors are read-only. The existing
initialized root must match an unambiguous reflected inline layout and a verified
`attribute-command-v1` executable profile. Inputs and calculated results must
satisfy the resolved finite scalar and signedness rules. The complete original
root program runs in descending order, and every resulting changed scalar joins
the native write mask, including dependencies outside the caller's selected
fields. Unsupported attribute layouts or programs reject the whole write.

Whole-component `write` retains padding and unselected fields from its original
source snapshot. The native CAS compares that complete snapshot and rejects
concurrent changes. These APIs do not supply unknown engine ownership operations,
publish events or establish replication and gameplay semantics.

## Original named service fields

`runtime.services.get("ecs::GameSettings")` follows the original named engine
DI binding and its separately proven mutable plain-data contract. Missing
contracts, unreflected names, dynamic or reference-owning layouts, and inactive
authority worlds return `nil, reason`. Reflection alone does not grant service
mutation. Views expose their canonical `type`, original `key`, `session_id` and
`scope="host-world-shared"`. Every consumer of that authoritative service shares
the effect of a field change.

`service:read_fields(paths)` reads a fresh snapshot using the common inline field
codec. `service:write_field(path, value, expected, expected_fields)` requires the
previous value of the selected field. The optional fourth argument is a nonempty
map of at most 1,024 service-relative expected fields. The existing
`expect_inline_fields` codec checks every named dependency against the fresh
snapshot. Missing fields, incompatible values, overlapping selections and changed
expectations reject the call. The native original Scene gate then compares the
complete service snapshot, writes only the selected value leaves and confirms
the resulting bytes and owner before returning.

Both service methods retain the original world session. Writes require
`runtime.ecs.write` and stop on the shared session fault after a genuinely
unconfirmed mutation. This API does not create engine allocation, notifications,
replication, persistence or an automatic UI refresh.

## Owned values

```lua
local ty = assert(runtime.types.resolve("keen::ecs::CurrentTransform"))
local value, reason = runtime.values.new(ty)
if value then
    local bytes = assert(runtime.values.encode(ty, value))
    local copy = assert(runtime.values.decode(ty, bytes))
end
```

`new` uses the reflected default; missing defaults return a reason instead of
inventing zero initialization. `encode` validates values and `decode` owns its
input bytes. They do not read process addresses or allocate engine objects.
The upstream KFC value codec cannot construct DS arrays, strings, optionals or
variants; these are rejected explicitly, including when nested inside a struct.
All of their metadata remains available through `runtime.types`.

## Functions and calculations

```lua
local operations = runtime.functions.get_operations()
local read, reason = runtime.functions.bind("runtime.ecs.read")
-- A returned binding is the existing checked Lua wrapper, not a raw native call.

local offset = 0
repeat
    local page, error = runtime.functions.list_native(offset, 256)
    if not page then break end
    for _, fn in ipairs(page.entries) do
        -- fn.rva, fn.ranges, fn.code_bytes, fn.callable, fn.reason
    end
    offset = page.next_offset
until offset == nil
```

The inventory combines the **loaded executable's** chained unwind records,
code pointers in data sections, and live component callback registrations. The
historical build-1076226 client / build-1024233 server snapshots contain
**52,150 / 38,565 code candidates** and 38,906 / 26,617 unwind procedure groups.
The KFC4 client capture for Steam build 25966634 contains **46,124 native
inventory entries** and 39,308 unwind procedure groups. A code pointer may point to
an internal label; its presence alone does not prove a complete function.
Unreferenced leaf functions and inlined code can still be absent.

Candidates without a unique original engine descriptor have an
`unclear_<eight-hex-digit-RVA>` name. Every entry has an
executable-scoped `key` (`SHA256/name`), ownership evidence, and separate
validation fields for address, signature, engine context and gameplay effects.
`list` and `get` are the canonical names; `get_native` selects the native
descriptor directly, while `get(operation_id)` also attaches the existing checked
operation wrapper. Listings include partial entries. `get` returns a binding even if its
native ABI remains unknown. All provisional native contracts remain incomplete.

`list`, `get`, and `find` use the same inventory and resolve every discovered
system dependency through the current parser registry. This path does not use
the short list of checked profile operations. Each original dependency slot
retains its `offset` and `entries`, and adds `resolved_entries`. Their `type`
is the canonical `Type` object from `runtime.types`, so its fields, declaring
types, inheritance, alignment and defaults follow the same parser contract.
The associated live registration exposes its original name, index,
`runtime_type`, `template_type`, and the dependency's `roles`. All names, hashes
and storage sizes must agree. Missing, invalid and ambiguous associations retain
an explicit reason and never become callable through this join.

```lua
local stamina = assert(runtime.types.resolve("keen::ecs::Stamina"))
local page = assert(runtime.functions.find({component=stamina}, 0, 256))
for _, fn in ipairs(page.entries) do
    for _, system in ipairs(fn.engine_descriptors or {}) do
        for _, slot in ipairs(system.dependency_slots) do
            for _, dependency in ipairs(slot.resolved_entries) do
                if dependency.type then
                    print(system.name, slot.offset, dependency.type.qualified_name)
                    for _, field in ipairs(dependency.type.fields) do
                        print(field.name, field.type.qualified_name, field.data_offset)
                    end
                end
            end
        end
    end
end
```

`find({name="inventory"})` filters by an original function-name substring.
Both filters can be combined and use the same `offset`, `limit`, `count` and
`next_offset` pagination as `list`. Component searches include callbacks owned
by that component and system dependencies on its original registration,
runtime layout or template layout. Each page's `dependency_resolution`
reports the global descriptor, dependency-reference and distinct-type counts,
with separate reflection and live-registration resolution totals.

The corrected KFC4 inventory for Steam build 25966634 contains **722 named system
descriptors and 2,904 dependency references to 586 distinct reflected types**.
All 586 types have an exact parser identity and a unique relation to the captured
live registration. The original six dependency spans include descriptor offset
`0x68`, whose omission previously hid 100 references and the entire
`copy_parent_level` descriptor. Runtime resolution reports its actual totals,
rather than treating these observations as a build requirement.

Original system-builder and scheduler evidence assigns `configuration-read` to
the two template spans (`0x18`, `0x28`), `read` to runtime spans `0x38`, `0x48`,
and `write` to runtime spans `0x58`, `0x68`. Each slot exposes `storage_role`,
`access_mode`, its original `system_record_mask_offset`, and `access_evidence`.
The evidence comes from `systemDependencyLayout` in the exact SHA256 profile.
Foreign images and profiles without this evidence retain null access roles.
Descriptor optionality remains unknown. Native signatures, execution permission
and gameplay effects are separate and remain unresolved by this metadata join.

```lua
local component = assert(runtime.ecs.get_component("keen::ecs::Extinguish"))
for _, callback in ipairs(component.callbacks) do
    local binding = assert(runtime.functions.get(callback.function_rva))
    print(binding.name, binding.key, binding.callable, binding.reason)
    -- An unknown binding is inspectable. Calling it returns nil,reason.
    -- Changing descriptor fields cannot change the captured implementation.
end
```

`get` also runs a bounded proof checker on the current code bytes. It accepts
only completely understood straight-line copies between two buffer pointer
arguments, optionally returning a constant. Calls, jumps, globals, stack use,
unproved memory accesses and missing returns are rejected. Accepted bodies
receive an **owned-buffer interpreter** with this contract:

```lua
local binding = assert(runtime.functions.get(callback.function_rva))
if binding.callable then
    local plan = binding.buffer_transform
    -- Supply owned binary strings. All unwritten destination bytes are retained.
    local output, reason, native_constant = binding.call(source_bytes, destination_bytes)
end
```

The adapter executes the proved byte transformations on Lua-owned strings. It
does not invoke an engine address, mutate a live component, allocate engine
objects, or assert a gameplay meaning. Its `execution` is
`owned-buffer-copy-interpreter` and `native_callable` remains false. Buffer
lengths are checked. Two registered Extinguish callbacks per examined target
passed the proof checker; the adapters matched isolated copies of the original
instructions in **1,024 randomized differential trials across both targets**,
including preserved-byte guards and source immutability. Other candidates can
be checked lazily by `get`; no hash/name whitelist selects the accepted code.

`bind("runtime.ecs.read")` returns the existing checked operation wrapper.
`bind(binding.name)` returns a proven adapter when available. For incomplete
bindings, `bind` returns `nil,reason`; their descriptors remain available via
`get`. Native signatures, server operation contexts, and calculations using
unproved instructions remain unresolved.

## Original engine identities and existing mods

The 2026-10-09 readers find **689 client / 552 server** named execution
descriptors in the loaded executable. These names enrich the existing function
entries; there is no separate system catalog in the Lua API. Dependency lists
retain their raw descriptor offsets because their access-mode semantics are not
fully established. An original name does not prove a callable native signature.

```lua
local fn = assert(runtime.functions.get("network_player_attributes"))
print(fn.name, fn.rva, fn.name_provisional) -- original engine name, false
local modifier = assert(runtime.functions.bind_modifier("refill_stamina"))
print(modifier.owner.name, modifier.effect, modifier.scope)
assert(modifier.set_enabled(true))
-- Restore this intervention when your mod unloads:
assert(modifier.set_enabled(false))
```

A modifier ID describes a ShroudForge intervention, not an invented engine
function name. Its owner is the actual unwind root and, where present, the
original named execution descriptor. The existing guarded code writer remains
the backend. `runtime.patch` is a compatibility interface to that same backend.
Modifiers require an exact image, the matching function/byte guards and, for
attribute interventions, matching current KFC IDs and storage indices.
The bound descriptor explicitly reports `backend_kind` and `execution` as
`guarded-profile-patch`, `profile_required=true`, and `native_callable=false`.
An absent or ambiguous owner returns a reason stating that a guarded profile
patch is required. Resolving its owner or attribute names does not convert an
existing patch mod into a generic ECS or native-call mod.

`runtime.patch.glider_pitch` is also available through the existing
`runtime.patch.available(name)` and `runtime.patch.set_enabled(name, enabled)`
calls. It requires an exact-image read profile. Its native resolver starts from
the original `actor_rotation` system and checks the decoded clamp instructions
and angle constants. An uncertain code write faults the operation for the current
process and is not automatically retried. This operation is
process-wide. The caller must disable it when its effect ends and coordinate
ownership with any other mod requesting the same patch. The resolver establishes
the selected machine-code contract, not a confirmed multiplayer gameplay effect.
Client and dedicated-server processes resolve and toggle their own intervention.

| Mod or historical package | Proven origin and resulting intervention |
| --- | --- |
| `sf-auto-stamina-refill` | `network_player_attributes`: `Stamina` (`0x04b6aa8b`) is assigned `Stamina_Max` (`0xf443c410`) before the network snapshot. This is replenishment, not proof that depletion calculations are skipped. Modifier `refill_stamina`. |
| `sf-no-fall-damage` | `fall_damage_infliction`: preserve `Health` (`0x8eb84995`) by suppressing its store and retaining the subsequent recalculation. The previous payload added the calculated value to Health. Modifier `preserve_health_on_fall`. |
| `sf-unlimited-flight` | Uses `runtime.patch.glider_pitch` to relax the original pitch boundary and the shared runtime API for `StaminaDepletionMod_Glider` and the four reflected `ItemInfo.equipment.gliderConfig` inputs. The pitch change applies to all gliding actors in the process. Live multiplayer behavior remains unverified. |
| Historical `sf-no-resource-cost` | Shared helper: force its sixth integer argument to zero. Proven callers include `actor_apply_buff`, `player_crafting`, `inventory_actions` and building systems. It affects every execution of the shared site. Modifier `zero_resource_argument`. |
| Historical `sf-infinite-item-use` | Forward false in place of a sixth boolean argument. Current operand tracing associates its counterpart with inventory insertion and actionbar selection, rather than proving an item-consumption switch. Historical modifier `clear_item_use_argument`. |
| Historical `sf-infinite-item-split` | Shared helper: suppress only the partial source-stack subtraction at `[rsi+4]`. The separate full-removal path still clears the stack. Modifier `preserve_split_source`. |
| `sf-crafting-essentials` | Uses loaded `RecipeRegistryResource.recipes[].craftingDuration.value`, named `ecs::GameSettings.factoryProductionSpeedFactor`, and `ActorSequenceResource` variants `PayUsageCost.consume` / `ApplyBuffEvent.consume`. These typed data changes use existing guarded scalar writers and reversible scopes. Global resource-cost suppression and partial-source preservation remain unimplemented. Native relationships and verification limits are recorded in the [current investigation](../evidence/runtime/2026-10-18-crafting-native-transactions.en.md). |
| `sf-unlock-blueprints` | Reads `GameKnowledgeQueryResourceDb`, query `Unlock_Flame_Altar_PK`, action `NPC_Flame_Hint01`, and uses the stored knowledge ID in `RecipeRegistryResource.recipes[].knowledgeRequirement`. Both current targets resolve `1715248921`; this is a Flame base-hint knowledge condition, not an unconditional unlock flag. No numeric ID remains in the mod. |
| `world-editor` | Existing checked `runtime.world.*` wrappers and ECS access remain canonical. Verified native operations are linked into their function descriptors. The editor manifest targets the client. Dedicated-server native addresses and actor-frame bindings are now present, including finish_building. They still require a live matching context; server gameplay effects and client/server mod RPC have not been tested or implemented respectively. |

The unnamed shared helpers retain provisional names. Their static caller paths
are evidence of reachability, not a complete dynamic call graph. The historical
Lua lifecycle checks cover six modifier contracts, including unload restoration.
They do not establish that those packages remain bundled or that the current
Crafting Essentials data paths have passed a live gameplay test.

## Attribute IDs, storage and calculation programs

For KFC4 client build **25966634**, `runtime.ecs.get_attributes()` resolves
**307 original IDs in 59 roots** from 17 freshly read attribute resources.
All 307 stored programs decode against the active `keen::AttributeOps` enum.
There are **66 nonempty programs**. The catalog also exposes the **59 reflected
attribute component layouts** once in `component_layouts`.

Attribute names and IDs come from the KFC resources' `debugNames` and `ids`.
They are not generated by hashing visible names or reflection type names.
The previous KFC3 catalog contained 304 IDs in 56 roots. Its reflected
`definition` GUID permitted a static association through FNV-1a of the 16
original GUID bytes. KFC4 attribute components omit that field, so the API
does not require or manufacture a replacement GUID.

`read_attribute(entity, nameOrId)` and `read_attributes(entity, nameOrId)` join
the requested root to the entity's actual initialized component header.
Candidate layouts come from the active reflection and must have the exact
inline scalar array length. The provider resolves their original type names
through engine registrations. The reader accepts exactly one snapshot whose
`rootId`, storage offset, element count and Initialized bit agree. Missing
and ambiguous matches return an error. Component names do not establish which
attribute root their storage contains.

The attribute catalog indexes original names and ordered root entries once.
Compatible live candidate components can be read through the optional native
multi-component operation, bounded to 64 selections and 1 MiB of owned payload.
It shares owner and entity validation, then checks each candidate's bytes and
current presence. All compatible initialized roots remain part of the normal
identity and uniqueness checks. These grouped reads do not establish an atomic
gameplay transaction across components. Providers without the checked batch
export report the candidate snapshot as unavailable. Lua signatures and guarded
write calculations are unchanged.

Header fields use their reflected widths. KFC3 uses `storageOffset` and
`storageSize` as 32-bit values with a 16-bit flags field. KFC4 uses a 16-bit
`storageOffset`, an 8-bit `storageCount` and 8-bit flags. Both count elements,
not bytes. The Initialized mask is taken from the reflected bitmask enum.
KFC4 native attribute adders independently confirm bit zero. A known KFC3
definition GUID is still checked when present. The selected component and
association are returned with a root snapshot, without process pointers.

```lua
local reported = false
return {
    update_interval_ms = 250,
    on_update = function()
        if reported then return end
        -- Resume the same incremental query until it returns a complete list.
        local entities, reason = runtime.ecs.query("keen::ecs::PlayerInput",
            "keen::ecs::Stamina", "keen::ecs::Health")
        if not entities or #entities == 0 then return end
        local complete = true
        for _, entity in ipairs(entities) do
            local stamina = runtime.ecs.read_attributes(entity, "Stamina")
            local health = runtime.ecs.read_attributes(entity, "Health")
            if stamina and health then
                shroudforge.log.info(tostring(entity) .. ": stamina=" ..
                    tostring(stamina.values.Stamina) .. "/" ..
                    tostring(stamina.values.Stamina_Max) .. ", health=" ..
                    tostring(health.values.Health) .. "/" .. tostring(health.values.Health_Max))
            else complete = false end
        end
        reported = complete
    end,
}
```

`Stamina` and `Health` carry `server_only` metadata. The active host scene is
reachable through the original SceneOwner list and has a separate execution
owner and entity manager from the client view. An external read-only capture
on 16 October 2026 verified 1,360 Health and 73 Stamina root snapshots in that
host table. The observed player with `PlayerInput` had Health 450/450 and
Stamina 137/143. Its Stamina Base/Adder were 110/33 and Health Base/Adder were
150/300. The query above identifies players by actual components and does not
hardcode that capture's Entity ID or imply a local-player selection.

The initial client-view capture had no authoritative Health/Stamina root
components. It contained `NetworkStamina`, `NetworkHealth` and
`NetworkPlayerAttributes`, which are separate reflected network snapshots.
In that client context, existing inline-field access can read the client input
entity's network values as follows:

```lua
local reported = false
return {
    update_interval_ms = 250,
    on_update = function()
        if reported then return end
        local entities, reason = runtime.ecs.query("keen::ecs::ClientPlayerInput",
            "keen::ecs::NetworkPlayerAttributes")
        if not entities or #entities == 0 then return end
        local complete = true
        for _, entity in ipairs(entities) do
            local value = runtime.ecs.read_fields(entity,
                "keen::ecs::NetworkPlayerAttributes",
                {"health", "healthMax", "stamina", "staminaMax", "movementSprint"})
            if value then
                shroudforge.log.info(tostring(entity) .. ": stamina=" ..
                    tostring(value.stamina) .. "/" .. tostring(value.staminaMax))
            else complete = false end
        end
        reported = complete
    end,
}
```

The host capture contained validated storage samples for 319 distinct types.
The client capture contained 161. Their union covers 384 registered storage
types. Unobserved types and nonmatching columns remain separate findings.
These captures were external process reads. They do not establish that a new
DLL or the shown Lua queries had already executed inside the game. See the
[dated capture report](../evidence/runtime/2026-10-16-kfc4-ecs-attribute-read.de.md).
The examples are retrying mod entry points. An incomplete query returns
`nil, reason` until its scan finishes and must be resumed on a later update.
It must not be asserted as an immediate startup failure.

The current KFC4 Stamina and Health roots both contain eight signed 32-bit
slots beginning at reflected component offset 8. Their stored programs are:

| Root | Current slot and program | Maximum slot and program | Recharge slot and program |
| --- | --- | --- | --- |
| Stamina | 0, `Clamp(Stamina, Stamina_Min, Stamina_Max)` | 2, `LoadRef(Stamina)`, minimum, previous maximum, `Stamina_Max_Base + Stamina_Max_Adder`, `ScaleToNewMax` | 5, `Clamp(Stamina_Recharge, 0, 65536)` |
| Health | 0, `Clamp(Health, Health_Min, Health_Max)` | 7, `LoadRef(Health)`, minimum, previous maximum, `Health_Max_Base + Health_Max_Adder`, `ScaleToNewMax` | 6, `Clamp(Health_Recharge, 0, 65536)` |

The other stored entries are inputs with empty calculation programs. These
programs explain why a maximum change can also rescale the current value.
They do not prove the full depletion, damage, recharge or network pipeline.

`evaluate_attributes` runs the owned calculation model and returns a trace of
`before_bits` and `after_bits`. Its historical KFC3 validation covered 65
nonempty programs, 4,160 Unicorn differential vectors and 53 client / 54 server
live root snapshots. The current exact KFC4 image has now been checked against
all three original interpreters in isolated Unicorn emulation. All 66 nonempty
programs matched 4,720 vectors, including finite boundary inputs and descending
root recomputations. All four `ScaleToNewMax` programs matched 312 vectors each.
The comparison found and fixed Float Clamp's selection of equal +0/-0 bounds.
Both older KFC3 corpora still pass. Nonfinite floats remain rejected by the
owned model. Calculation authorization and native process-write authorization
are separate gates.

`update_attribute` requires that exact-build authorization in addition to
verified entity storage and the existing guarded compare-exchange path. It
rechecks the entity handle, initialized root, optional definition GUID, inline
layout, scalar domain and complete component snapshot. A conflict returns an
error. Its optional fourth parameter `expected_values` is a nonempty map of
original attribute names and expected numbers from the selected root. Every name
must belong to that same root. These values are checked against the fresh snapshot
before the original root calculation. The complete native component CAS still
rejects later changes. For example, a mod can update `Stamina_Min` to the observed
`Stamina_Max` only while both observed bounds remain unchanged:

```lua
local root, reason = runtime.ecs.read_attributes(entity, "Stamina")
if root then
    local values = root.values
    local updated, reason = runtime.ecs.update_attribute(entity, "Stamina_Min",
        values.Stamina_Max,
        {Stamina_Min=values.Stamina_Min, Stamina_Max=values.Stamina_Max})
end
```

The entity-read contract alone does not authorize process writes.
Original mutable accessors also register changes before returning a pointer.
Their generic tracker uses engine component indices, entity storage metadata
and the entity manager's tracking context. Recalculation alone does not reproduce
that contract. `write_attribute_storage`, `write_fields` and whole-component
`write` all run the shared original root calculation for AttributeComponent
subtypes before the complete-snapshot CAS. The storage writer therefore updates
an input through the same rules as `update_attribute`, including derived scalar
changes. None of these calls synthesizes gameplay events. When the original
tracking backend is enabled, it validates the original tracker before mutation.
The current exact KFC4 write profile admits only components whose original A
and B tracking registration bits are both clear, under the registered local
authority lifetime and Scene execution gates. Tracked components remain
unavailable. This decision is made from the current engine registration, not a
mod-specific type list. `write_fields(..., expected_fields)` can additionally
compare caller-selected expected values using the same reflected codecs before
the complete native snapshot CAS.
An unconfirmed rollback blocks further generic ECS, named-service and
session-bound resource writes in that same native binding epoch. Ordinary CAS
conflicts and busy engine gates do not set this fault. A real world or registry
rebinding invalidates the fault together with the old handles and snapshots.
`storage_writable`,
`calculation_writable`, `write_value_domain` and their reasons describe the
resolved scalar contract, not permission to bypass provider write gates.

## Client/server execution and module boundaries

The same Lua names bind to the **current process**, using its executable profile,
fresh KFC data and live registrations. That is not an RPC connection. A local
write is not a guarantee of an authoritative or persistent multiplayer change.
Original `server_only` component metadata is exposed as `engine_server_only`.

| Existing mod | Client/server consequence |
| --- | --- |
| Stamina | The current mod uses named attribute roots. Writable context is limited to validated local authority. Remote server state can overwrite client changes. |
| Fall damage | The current mod resolves each actual player template and its original FallDamage variant. Shared templates share the effect. Resource writing requires the separately validated exact-image contract. |
| Flight | Same rotation system intervention; prediction and authoritative movement can still disagree when configured on only one side. |
| Resource cost / item use / item split | Shared helper sites are mapped separately per executable. The process handling the inventory transaction governs its result; matching code does not prove end-to-end network equivalence. |
| Blueprints | KFC asset transformation resolves the same original query/action on both targets. Client UI and server recipe/knowledge checks must use compatible data. |
| World Editor | Client cursor/UI, capture and blueprint storage remain client-side. Singleplayer uses the existing direct native path; a client explicitly targeting a dedicated server sends the existing SFBP V7 blueprint over `runtime.network.send_mod`, and the headless server target executes native paste/undo locally. Replication and persistence remain live-test requirements. See [the complete per-mod audit](../modding/multiplayer-and-target-processes.de.md). |

Server world context now comes from `player_building_place_prop`'s execution
frame, not a copied client singleton. Its pointer is bounded/checked at use,
expires after 500 ms without a fresh observation, and is reset when the entity
manager changes or the runtime shuts down. Place, Destroy and both voxel bodies
match the client's instruction structure after relocation; Spawn has equivalent
operand flow. Server finish is RVA `0x1c2dc0`, with the original event slot at
`0xb57140`. Zero is a valid supplied event value on both targets. The fallback
Destroy path now forwards the material feedback ID rather than the tracking ID.
The server has no cursor hook. Native code/ABI checks do not replace a live
server gameplay test after installation.

| Responsibility | Source |
| --- | --- |
| Shipped build evidence | `modules/game-runtime/profiles/`: executable identity, engine layouts/hooks, checked native operations and interventions. No duplicated 517-component tables. |
| Automatic resolution | `modules/mod-runtime/reflection/`: GUID/ID/definition joining, type traversal and owned calculation model. No Lua, process access or diagnostic file I/O. Native `modules/game-runtime/native/ecs/registry/read_component_registry.h` reads the actual engine registrations. |
| Lua and mod runtime | `modules/mod-runtime/lua/ingame/`: one public runtime API, permission/lifecycle checks and adapters to the native game-thread dispatcher. |
| Developer diagnostics | `dev/profiles/`: extraction, audits and differential emulation. Never required by a user's modloader installation. |

Both profiles use `componentResolution: live-registration`. ECS access waits for
the proven engine registry instead of guessing from stale inline indices. Legacy
profile fallback is still readable for compatibility. EML/KFC pregame APIs retain
their existing separate namespace; this change does not add a second mod API.

## Build and verification

Build both parts together:

```powershell
cmake --build build/native-runtime --config Release --target kfc-runtime
cargo build --release -p shroudforge-modloader --lib
```

Artifacts: `build/native-runtime/bin/kfc-runtime.dll` and
`target/release/shroudforge_modloader.dll` (installed as `shroudforge-runtime.dll`).
Both updated DLLs are needed. Already running games retain their loaded code;
installing the pair and restarting activates the new API. No restart or live
DLL replacement was performed during the read-only verification.

The normal discovery runner now writes `components.json` in addition to
reflection/code evidence. Full captured registries and production-reader checks
are under `dev/profiles/captures/registry-complete-20261008/`.
See the [profile development guide](../../dev/profiles/README.md)
for commands and [investigation](../evidence/runtime/2026-10-08-component-registry.md)
for the layout evidence.

Full build catalogs are exported to each target's `runtime-bindings.json`. The
small committed indexes are
`dev/profiles/catalogs/enshrouded/client/1076226.runtime-index.json`
and `dev/profiles/catalogs/enshrouded/server/1024233.json`. Their
`catalog` paths lead to the complete local generated artifacts. These discovery
indexes are not executable compatibility profiles. The executable server profile is now
`modules/game-runtime/profiles/enshrouded/server/1024233.json`, matching its actual
KFC version. When replacing an external installed profile, remove the obsolete
server `1076226.json`; retaining both produces an ambiguous exact match. The
client profile remains `client/1076226.json`. Both are pinned by executable SHA256.
