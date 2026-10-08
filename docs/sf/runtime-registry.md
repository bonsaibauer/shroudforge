# Runtime registry API

The Lua interface has four layers. Every layer uses the current process's type
registry. Client and dedicated server expose the same API shape.

| Namespace | Contract |
| --- | --- |
| `runtime.types` | All reflected types, hashes, inheritance, fields, enums, attributes, flags and default bytes. This is the same object as `game.types`. |
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
assert(runtime.types.resolve(ty.qualified_hash) == ty)
local same = assert(runtime.types.resolve({hash = ty.qualified_hash, domain = "qualified"}))
local collisions = runtime.types.find_by_hash(ty.internal_hash, "internal")

for _, field in ipairs(ty.fields) do
    print(field.declaring_type.qualified_name, field.name,
          field.type.qualified_name, field.data_offset)
end
```

Selectors accepted by type/value/ECS APIs are a qualified name, a `Type`, an
unsigned qualified hash, or `{hash, domain}`. Domains are `qualified`, `internal`,
`name`, and `impact`. Ambiguous hashes never select the first match. Bare
integers always mean a qualified hash, never a pointer or component index.

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
print(component.index, component.qualified_hash)
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
or hash and select its actual runtime layout. A template-only registration has
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

Every candidate has a provisional `unclear_<eight-hex-digit-RVA>` name, an
executable-scoped `key` (`SHA256/name`), ownership evidence, and separate
validation fields for address, signature, engine context and gameplay effects.
`list` and `get` are the canonical names; `list_native` and `get_native` are
aliases. Listings include partial entries. `get` returns a binding even if its
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
indexes are not executable compatibility profiles. The legacy server profile
filename `1076226.json` is not used to identify the server's KFC data version.
