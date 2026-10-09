# Runtime registry API

The Lua interface has four layers. Every layer uses the current process's type
registry. Client and dedicated server expose the same API shape.

| Namespace | Contract |
| --- | --- |
| `runtime.types` | All reflected types, inheritance, fields, enums, attributes, flags and default bytes. This is the same object as `game.types`. |
| `runtime.values` | Decode, validate and encode owned KFC values using those layouts. |
| `runtime.ecs` | Engine registrations, their runtime/template layouts, and checked access to entity storage. |
| `runtime.functions` | Typed operations and build-scoped provisional bindings for all evidenced native code candidates. |

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

The engine has **598 registrations** in each examined executable. Of these,
**517 have entity storage**, **81 have template configuration only**, and **60
have both storage and a separate configuration type**. The total configuration
layout count is 141. A null storage pointer in the 81 entries is accompanied by
a valid configuration type and storage size zero. It is not an unresolved
component to which a size or address should be assigned.

The provider follows the entity manager to its registration owner, checks the
record array and bounded length, then validates every name/hash/type/size against
reflection and both parallel storage arrays. It rereads registration identity
before publishing. This resolves original names and `Dynamic*` storage names
without a component profile. The structural reader currently understands layout
version 1. Unsupported layouts fail validation; matching sizes alone never
create a mapping. Existing exact-build component profiles remain a fallback
until an engine registry has been validated.

`query`, `query_bounds`, `read`, and `write` accept the original registration name
or its returned `Type` and select its actual runtime layout. A template-only registration has
no entity column, so it cannot be read or written through entity storage calls.
Its configuration can be inspected through reflection and the existing KFC asset
API; this change does not implement live template replacement or engine allocation.

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
examined snapshots contain **52,150 client / 38,565 server code candidates**;
38,906 / 26,617 of those are unwind procedure groups. A code pointer may point to
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

| Bundled mod | Proven origin and resulting intervention |
| --- | --- |
| `sf-auto-stamina-refill` | `network_player_attributes`: `Stamina` (`0x04b6aa8b`) is assigned `Stamina_Max` (`0xf443c410`) before the network snapshot. This is replenishment, not proof that depletion calculations are skipped. Modifier `refill_stamina`. |
| `sf-no-fall-damage` | `fall_damage_infliction`: preserve `Health` (`0x8eb84995`) by suppressing its store and retaining the subsequent recalculation. The previous payload added the calculated value to Health. Modifier `preserve_health_on_fall`. |
| `sf-unlimited-flight` | `actor_rotation`: replace one scalar load with `-1.57f`. The original variable name remains unresolved. The server's invalid RIP-relative constant reference is fixed and represented explicitly in the payload profile. Modifier `override_rotation_constant`. |
| `sf-no-resource-cost` | Shared helper: force its sixth integer argument to zero. Proven callers include `actor_apply_buff`, `player_crafting`, `inventory_actions` and building systems. It affects every execution of the shared site. Modifier `zero_resource_argument`. |
| `sf-infinite-item-use` | Shared helper: forward false in place of its sixth boolean argument. Call paths include `inventory_actions`, `actor_pay_usage_cost`, `equipment_actions` and `actor_spawn_entity`. Modifier `clear_item_use_argument`. |
| `sf-infinite-item-split` | Shared helper: suppress the source-stack subtraction at `[rsi+4]`. Multiple inventory, equipment, crafting and spawn systems reach it. Modifier `preserve_split_source`. |
| `sf-unlock-blueprints` | Reads `GameKnowledgeQueryResourceDb`, query `Unlock_Flame_Altar_PK`, action `NPC_Flame_Hint01`, and uses the stored knowledge ID in `RecipeRegistryResource.recipes[].knowledgeRequirement`. Both current targets resolve `1715248921`; this is a Flame base-hint knowledge condition, not an unconditional unlock flag. No numeric ID remains in the mod. |
| `world-editor` | Existing checked `runtime.world.*` wrappers and ECS access remain canonical. Verified native operations are linked into their function descriptors. The editor manifest targets the client. Dedicated-server native addresses and actor-frame bindings are now present, including finish_building. They still require a live matching context; server gameplay effects and client/server mod RPC have not been tested or implemented respectively. |

The unnamed shared helpers retain provisional names. Their static caller paths
are evidence of reachability, not a complete dynamic call graph. Lua lifecycle
tests cover all six modifier mods on both profile contracts, including unload
restoration. These tests do not assert a fresh live gameplay test of every mod.

## Attribute IDs, storage and calculation programs

`runtime.ecs.get_attributes()` resolves **304 original IDs in 56 roots** from
both current KFC containers. All 304 now have a unique component association.
Root IDs use FNV-1a over the **16 definition-GUID bytes** (zero GUID maps to
zero), as implemented by the native initializers. They are not hashes of the
visible names or reflection type hashes. This resolves container-only
`ManaRechargeMod` without inventing a standalone resource.

All EXE types and the 59 relevant attribute/balancing/knowledge resources were
read freshly for each target, including the `.bak` resource pair. Current and
backup values agree for these resources. `BodyHeat` and `FreezingResistance`
still have different KFC and reflected signedness. This is not a stale-cache
finding. Type-cache identity now includes EXE SHA256 and extractor revision.

```lua
local stamina = assert(runtime.ecs.get_attribute("Stamina"))
local same = assert(runtime.ecs.get_attribute(stamina.hash))
local entities = assert(runtime.ecs.query("keen::ecs::Stamina"))
for _, entity in ipairs(entities) do
    local snapshot = assert(runtime.ecs.read_attributes(entity, "Stamina"))
    snapshot.values.Stamina_Max_Base = 200
    local preview = assert(runtime.ecs.evaluate_attributes("Stamina", snapshot.values))
    print(preview.values.Stamina_Max, preview.values.Stamina)
    -- Uses a new live snapshot, recalculates the whole root, then rejects a
    -- changed snapshot before writing on the game thread.
    local applied, reason = runtime.ecs.update_attribute(entity, "Stamina_Max_Base", 200)
    if not applied then print(reason) end
end
```

`read_attribute` reads one value. `read_attributes` reads the related root.
`evaluate_attributes` operates on owned values and returns values plus a trace
of each calculation's `before_bits` and `after_bits`, ordered by root index.
`update_attribute` uses the same calculation model, checks the entity handle,
root, GUID, initialized flag, inline layout and scalar domain, and compares the
entire component snapshot before the masked write. A conflict returns an error;
it does not silently overwrite a newer calculation. The optional native
`KfcRuntimeEcsCompareExchange` export is required; older providers reject the
new update call. Writes and comparisons share one game-thread dispatch, not a
CPU atomic transaction against arbitrary engine worker threads.

The model covers **all 65 nonempty calculation programs** in the current
304-entry corpus. Roots run in descending index order. `LoadRef` is an actual
reference consumed by `ScaleToNewMax`, which can change another entry before
its own calculation runs. Integer operations wrap at 32 bits. Float operations
round at f32 precision, and float `Push` converts an unsigned integer literal
instead of interpreting its bits as IEEE float data. Original `AttributeOps`
names, stored words and references remain available on the descriptor.
Unsupported instructions, stack underflow, invalid references, excessive
programs and non-finite numbers are rejected. This is not an implementation of
all possible future AttributeOps programs or the rest of the engine VM.

Validation uses **4,160 differential vectors** against both original x64
interpreters inside Unicorn. In addition, **53 client / 54 server** live
attribute-root snapshots passed the production Lua storage validator, including
BodyHeat and FreezingResistance; malformed roots/offsets were rejected. The game EXEs are read-only emulator data, never
loaded as host code. The native recompute loop establishes descending order.
`attributeCalculationModel` authorizes live updates only on an exact SHA256
profile. Generating a profile for another build removes this authorization
until its calculations are independently checked.

`FreezingResistance` is initialized through the signed native interpreter.
Its current `Base + Heat_Source` program uses identical 32-bit arithmetic in
both integer variants, so it supports the general update API. Conflicting
signedness permits inputs only in the common range `0..2147483647`:

```lua
local updated, reason = runtime.ecs.update_attribute(entity, "FreezingResistance_Base", 30)
```

This sets an attribute and recomputes its root; it does not establish that 30
means immunity, seconds, or any particular in-game protection. The consuming
systems and gameplay effect still need validation. **296 entries** support
root updates. The **8 BodyHeat entries** have signedness-dependent calculations
and remain blocked for that operation: initialization calls the unsigned helper,
while `body_heat_max_scaling` calls the signed helper. All 304 entries remain
readable; the explicitly low-level `write_attribute_storage` supports validated
scalar writes, with the common-range restriction for the 11 conflicting entries.
It does not recalculate or publish events. `storage_writable`,
`calculation_writable`, `write_value_domain` and reasons distinguish these cases.

## Client/server execution and module boundaries

The same Lua names bind to the **current process**, using its executable profile,
fresh KFC data and live registrations. That is not an RPC connection. A local
write is not a guarantee of an authoritative or persistent multiplayer change.
Original `server_only` component metadata is exposed as `engine_server_only`.

| Existing mod | Client/server consequence |
| --- | --- |
| Stamina / fall damage | Same stored attribute IDs and named owner systems on both builds. Server-owned state and later snapshots can overwrite a remote client's local changes. |
| Flight | Same rotation system intervention; prediction and authoritative movement can still disagree when configured on only one side. |
| Resource cost / item use / item split | Shared helper sites are mapped separately per executable. The process handling the inventory transaction governs its result; matching code does not prove end-to-end network equivalence. |
| Blueprints | KFC asset transformation resolves the same original query/action on both targets. Client UI and server recipe/knowledge checks must use compatible data. |
| World Editor | Client cursor/UI remains client-side. In addition to direct world calls, `runtime.world.building.input` now queues ordinary ClientPlayerInput actions through the game's existing input path. This requires no separate mod transport; network acceptance, replication and persistence remain unverified with the new adapter. See [the complete per-mod audit](mod-multiplayer.md). |

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
| Shipped build evidence | `src/loader/runtime/profiles/`: executable identity, engine layouts/hooks, checked native operations and interventions. No duplicated 517-component tables. |
| Automatic resolution | `api/src/runtime_resolution/`: GUID/ID/definition joining, type traversal and owned calculation model. No Lua, process access or diagnostic file I/O. Native `component_registry.h` reads the actual engine registrations. |
| Lua and execution | `api/src/env/runtime_*`: one public runtime API, permission/lifecycle checks and adapters to the native game-thread dispatcher. |
| Developer diagnostics | `runtime/profile-tools/dev/`: extraction, audits and differential emulation. Never required by a user's modloader installation. |

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
are under `src/loader/runtime/profile-tools/devdata/registry-complete-20261008/`.
See the [discovery guide](../../src/loader/runtime/profile-tools/dev/DISCOVERY.md)
for commands and [investigation](../../src/loader/runtime/profile-tools/dev/investigations/2026-10-08-component-registry.md)
for the layout evidence.

Full build catalogs are exported to each target's `runtime-bindings.json`. The
small committed indexes are
`profile-tools/dev/function-catalogs/enshrouded/client/1076226.runtime-index.json`
and `profile-tools/dev/function-catalogs/enshrouded/server/1024233.json`. Their
`catalog` paths lead to the complete local generated artifacts. These discovery
indexes are not executable compatibility profiles. The executable server profile is now
`src/loader/runtime/profiles/enshrouded/server/1024233.json`, matching its actual
KFC version. When replacing an external installed profile, remove the obsolete
server `1076226.json`; retaining both produces an ambiguous exact match. The
client profile remains `client/1076226.json`. Both are pinned by executable SHA256.
