# Enshrouded Mod Loader (EML) API Reference & Syntax Dictionary

> **Version:** 0.0.2
> **Target Runtime:** `emm.exe` / EML Lua runtime
> **Environment:** Client and server modding engine
> **Primary Sources:** `base.lua` runtime definitions plus the EML documentation and runtime-tested examples

---

## Purpose of this document

This reference is intended for AI systems, toolchains, and human authors generating Enshrouded EML Lua code. It is written as a strict implementation guide so that generated code is more likely to be valid, self-contained, and aligned with the runtime's actual constraints.

Use this document as a contract, not as loose inspiration.

---

## Core rules for AI-generated EML code

The following rules are essential and should be treated as mandatory when writing or reviewing EML mods.

1. Prefer a single self-contained file
   - The normal mod entrypoint is `src/mod.lua`.
   - Keep logic, helper functions, and data formatting inside that file.
   - Avoid introducing extra files unless the user explicitly asks for them.

2. Respect the sandbox
   - EML mods run in a restricted runtime environment.
   - Do not assume direct access to arbitrary external files, processes, or system APIs beyond the documented EML APIs.
   - Use `io.read`, `io.read_to_string`, `io.export`, and related APIs only when appropriate and permitted by runtime flags.

3. Do not confuse in-memory exports with full game data dumps
   - Many outputs that look like “full data” are only snapshots of what is currently loaded in memory.
   - `game.assets` queries typically expose currently active runtime objects, not the entire underlying disk archive.
   - For deeper or off-memory assets, you may need reflection data, GUID resolution, and content streaming.

4. Check feature flags before using export or runtime features
   - `io.export()` is only appropriate when `loader.features.export` is enabled.
   - Runtime DLL registration is only appropriate when `loader.features.runtime.dll` is enabled.

5. Use reflection when you need schema or type information
   - If you need to inspect a type, field layout, enum values, or metadata, use the runtime-exposed `game.types` object and `Type` descriptors.
   - Avoid hard-coding engine structures unless you have evidence from reflection or the runtime definitions.

6. Prefer defensive coding for runtime interactions
   - Use `pcall()` around risky calls where possible.
   - Handle `nil` and missing data gracefully.
   - Do not assume every asset, field, or content lookup will succeed.

7. Keep output human-readable and well-structured
   - When exporting data, use clear keys, labels, and formatting.
   - If the goal is debugging or data mining, include progress logging and stable output names.

---

## Verified runtime-tested snippet patterns

The following snippets were validated in the runtime test pass and should be preferred when generating new EML code.

### Safe loader / export check

```lua
if loader and loader.features and loader.features.export then
    print("Export enabled")
else
    print("Export disabled")
end
```

### Safe asset discovery pattern

```lua
local ok, resources = pcall(function()
    return game.assets.get_resources_by_type("keen::ItemInfo")
end)

if ok and type(resources) == "table" then
    for _, res in ipairs(resources) do
        print(res.guid or "unknown")
    end
end
```

### Defensive reflection pattern

```lua
local ok, resolved = pcall(function()
    return game.types.get_by_qualified_name("keen::ItemInfo")
end)

if ok and resolved ~= nil then
    print(resolved.qualified_name or "unknown")
end
```

### Buffer pattern that passed in testing

```lua
local buf = buffer.create(0)
buf:write_string("hello")
local data = buf:read_string(5)
print(data)
```

### Utility pattern that passed in testing

```lua
local hashed = game.guid.hash("12345678-1234-1234-1234-1234567890ab")
local hash32 = hasher.fnv1a32("demo")
local hash64 = hasher.crc64("demo")
print(hashed, hash32, hash64)
```

---

## Table of contents

1. [Runtime context and mod structure](#1-runtime-context-and-mod-structure)
2. [Mod loader and runtime (`loader`)](#2-mod-loader-and-runtime-loader)
3. [File system and I/O (`io`)](#3-file-system-and-io-io)
4. [Asset, content, and resource APIs](#4-asset-content-and-resource-apis)
5. [Reflection and engine type system (`TypeRegistry`, `Type`)](#5-reflection-and-engine-type-system-typeregistry-type)
6. [Fixed-width integer primitives (`integer.*`)](#6-fixed-width-integer-primitives-integer)
7. [Buffer API](#7-buffer-api)
8. [System utilities (`builtin`, `GuidHelper`, `hasher`)](#8-system-utilities-builtin-guidhelper-hasher)
9. [Common AI mistakes to avoid](#9-common-ai-mistakes-to-avoid)

---

## 1. Runtime context and mod structure

EML mods are loaded from a mod package that typically contains:

```text
<game_dir>/mods/<mod_id>/
  mod.json
  src/
    mod.lua
```

### Expected execution model

- The runtime executes the mod entrypoint from `src/mod.lua`.
- The script is sandboxed and should not depend on external Lua modules or arbitrary OS-level operations.
- The runtime exposes helper globals and modules such as `loader`, `io`, `game.types`, `game.guid`, `hasher`, and the integer modules.

### Typical runtime flags

- `enable_console`: when enabled, runtime progress and print output are visible in the console window.
- `use_export_flag`: when enabled, data exported through the EML export system is written to the export directory.

### Important operational guidance

- If a task says “export data” or “dump data”, the most reliable approach is to write a script that inspects the currently loaded runtime state and emits structured output.
- If the task is to inspect game data beyond currently loaded objects, the agent should acknowledge that full-disk extraction is not the same as in-memory export and should use reflection or content resolution where appropriate.

---

## 2. Mod loader and runtime (`loader`)

The `loader` object exposes runtime state, feature availability, and limited runtime registration capabilities.

### Global instance

```lua
loader = {
    is_client = boolean,
    is_server = boolean,
    features = LoaderFeatures,
    runtime = LoaderRuntime
}
```

### `LoaderFeatures`

| Field | Type | Description |
| --- | --- | --- |
| `patch` | `boolean` | Indicates whether runtime patching is active. |
| `export` | `boolean` | Enables `io.export()` and related export behavior. |
| `runtime` | `RuntimeFeatures?` | Runtime capability flags. May be `nil` if runtime features are disabled. |

### `RuntimeFeatures`

| Field | Type | Description |
| --- | --- | --- |
| `dll` | `boolean` | Enables DLL registration through `loader.runtime.register_dll()`. |

### `LoaderRuntime`

#### `loader.runtime.register_dll(path)`

Registers a native `.dll` relative to the mod root. This is runtime-dependent and may be non-functional or stubbed in some builds.

- Parameters: `path` (`string`)
- Throws on missing/unreadable file or if DLL support is disabled.

### Runtime helper functions

#### `loader.has_mod(mod_id)`

- Parameters: `mod_id` (`string`)
- Returns: `boolean`
- Purpose: checks whether a named mod is currently loaded

### Recommended usage pattern

```lua
if loader and loader.features and loader.features.export then
    print("Export enabled")
else
    print("Export disabled")
end
```

---

## 3. File system and I/O (`io`)

The `io` module provides sandbox-safe file and path operations.

### Core functions

| Function | Signature | Return | Description |
| --- | --- | --- | --- |
| `io.read` | `(path: string)` | `Buffer` | Reads a file as a raw binary buffer. |
| `io.read_to_string` | `(path: string)` | `string` | Reads file contents as UTF-8 text. |
| `io.list_files` | `(path: string)` | `string[]` | Lists entries in a directory. |
| `io.exists` | `(path: string)` | `boolean` | Checks whether a path exists. |
| `io.is_file` | `(path: string)` | `boolean` | Checks whether the path points to a file. |
| `io.is_directory` | `(path: string)` | `boolean` | Checks whether the path points to a directory. |
| `io.name` | `(path: string)` | `string` | Returns the filename or trailing directory name. |
| `io.name_without_extension` | `(path: string)` | `string` | Returns the filename without its extension. |
| `io.extension` | `(path: string)` | `string` | Returns the extension without the leading dot. |
| `io.parent` | `(path: string)` | `string|nil` | Returns the parent directory or `nil` at the root. |
| `io.join` | `(...: string)` | `string` | Joins path segments into a normalized path string. |
| `io.export` | `(path: string, bytes: string|Buffer)` | `Buffer` | Writes data to the export location when export is enabled. |

### Export safety rules

- `io.export()` should only be used when `loader.features.export` is enabled.
- Export behavior is constrained to the configured export area.
- Writing outside the allowed export boundary should be treated as unsafe and may raise runtime errors.

### Recommended pattern

```lua
if loader.features and loader.features.export then
    local ok = pcall(function()
        io.export("example.txt", "hello from eml")
    end)
    if not ok then
        print("Export failed")
    end
end
```

---

## 4. Asset, content, and resource APIs

The runtime exposes asset access through an `AssetManager`-style interface. In practice it is commonly used via `game.assets` in EML scripts.

### `AssetManager`

#### `get_resource(guid, type, part)`

- Returns a resource by GUID, type, and optional part.
- `type` may be a string or a `Type` object.

#### `get_resource_parts(guid, type)`

- Returns all parts of a resource with the given GUID and type.

#### `get_resources_by_type(type)`

- Returns all resources of the requested type.
- Useful for scanning currently loaded data.

#### `get_all_resources()`

- Returns all resources currently visible to the runtime.

#### `get_resource_types()`

- Returns all visible resource types.
- Good for discovery and debugging.

#### `create_resource(value, type[, guid, part])`

- Creates new runtime resources.
- Requires the target type to be registered and compatible with the supplied value.

#### `get_content(guid)`

- Resolves a content object from a GUID or content hash.
- Returns a `Content` object or `nil`.

#### `get_all_contents()`

- Returns all content objects visible to the runtime.

#### `create_content(data)`

- Creates a new content object from raw bytes.

### `Content`

A `Content` object represents raw binary game content.

#### `Content:read_data()`

- Returns a read-only `Buffer` containing the raw bytes of the content.

### `Resource`

A `Resource` exposes data and metadata for a runtime asset.

| Field | Type | Description |
| --- | --- | --- |
| `guid` | `Guid` | Resource identifier |
| `type` | `Type` | Runtime reflection type |
| `part` | `u32` | Resource part number |
| `data` | `unknown` | Structured data for the asset |
| `original_data` | `unknown` | Read-only original data reference when available |

### Important asset guidance for AI

- Do not assume `get_all_resources()` or `get_resources_by_type()` yields every asset in the game. These are runtime-visible objects.
- For data mining, output may look “complete” but often reflects only currently loaded memory state.
- If you need to work with off-memory assets, use content GUIDs, hashes, and content streaming rather than assuming all assets are immediately accessible.

### Example pattern

```lua
local ok, resources = pcall(function()
    return game.assets.get_resources_by_type("keen::ItemInfo")
end)

if ok and type(resources) == "table" then
    for _, res in ipairs(resources) do
        print(res.guid, res.type and res.type.qualified_name or "unknown")
    end
end
```

---

## 5. Reflection and engine type system (`TypeRegistry`, `Type`)

Reflection is one of the most important APIs in EML. It allows scripts to inspect engine types, fields, enums, and metadata dynamically.

### `TypeRegistry`

The primary access point for reflection data in this runtime build is `game.types`.

```lua
-- Lookup by qualified name or hash
game.types.get(qualified_hash: u32) -> Type
game.types.get(qualified_name: string) -> Type

-- Specialized lookups
game.types.get_by_qualified_hash(qualified_hash: u32) -> Type
game.types.get_by_impact_hash(impact_hash: u32) -> Type
game.types.get_by_qualified_name(qualified_name: string) -> Type
game.types.get_by_impact_name(impact_name: string) -> Type

-- Reflection queries
game.types.get_all() -> Type[]
game.types.of(value: any) -> Type
```

### `Type`

A `Type` object describes an engine type.

```lua
{
    name = string,
    impact_name = string,
    qualified_name = string,

    name_hash = u32,
    impact_hash = u32,
    qualified_hash = u32,
    internal_hash = u32,

    namespace = string[],
    inner_type = Type?,
    size = u32,
    alignment = u32,
    element_alignment = u32,
    field_count = u32,
    primitive_type = PrimitiveType,
    flags = TypeFlag[],

    struct_fields = table<string, StructField>,
    enum_fields = table<string, EnumField>,
    attributes = table<string, Attribute>,

    default_value = unknown
}
```

### Field and attribute structures

#### `StructField`

| Field | Type | Description |
| --- | --- | --- |
| `name` | `string` | Field name |
| `type` | `Type` | Field value type |
| `data_offset` | `u32` | Offset inside the containing structure |
| `attributes` | `table<string, Attribute>` | Attached metadata |

#### `EnumField`

| Field | Type | Description |
| --- | --- | --- |
| `name` | `string` | Enum member name |
| `value` | `u64` | Enum member value |

#### `Attribute`

| Field | Type | Description |
| --- | --- | --- |
| `name` | `string` | Attribute name |
| `namespace` | `string[]` | Namespace path |
| `type` | `Type?` | Optional attribute type |
| `value` | `string` | Attribute payload |

### Primitive types

```lua
"None" | "Bool" | "UInt8" | "SInt8" | "UInt16" | "SInt16" | "UInt32" | "SInt32" | "UInt64" | "SInt64" | "Float32" | "Float64" | "Enum" | "Bitmask8" | "Bitmask16" | "Bitmask32" | "Bitmask64" | "Typedef" | "Struct" | "StaticArray" | "DsArray" | "DsString" | "DsOptional" | "DsVariant" | "BlobArray" | "BlobString" | "BlobOptional" | "BlobVariant" | "ObjectReference" | "Guid"
```

### Type flags

```lua
"None" | "IsDs" | "HasBlobArray" | "HasBlobString" | "HasBlobOptional" | "HasBlobVariant" | "IsGpuUniform" | "IsGpuStorage" | "IsGpuConstant"
```

### Reflection usage guidance

- Use reflection to discover the runtime schema before trying to interpret arbitrary object data.
- If a resource contains nested data, inspect `resource.type` and its `struct_fields` before assuming a field layout.
- When analyzing enums, inspect `enum_fields` rather than guessing names from raw numeric values.

### Example pattern

```lua
local ok, t = pcall(function()
    return game.types.get_by_qualified_name("keen::ItemInfo")
end)

if ok and t ~= nil then
    print(t.qualified_name)
    for name, field in pairs(t.struct_fields or {}) do
        print(name, field.type and field.type.qualified_name or "unknown")
    end
end
```

---

## 6. Fixed-width integer primitives (`integer.*`)

The integer modules (`integer.u8`, `integer.u16`, `integer.u32`, `integer.u64`, `integer.i8`, `integer.i16`, `integer.i32`, `integer.i64`) provide Rust-style integer semantics with explicit overflow handling.

Use `{TYPE}` as a placeholder for one of the supported integer types.

### Module structure

```lua
integer.{TYPE} = {
    MIN = {TYPE},
    MAX = {TYPE},
    BITS = u32
}
```

### Common methods

| Method | Signature | Description |
| --- | --- | --- |
| `parse` | `(value: string, radix?: u32) -> {TYPE}|nil` | Parses a string into an integer; returns `nil` on failure. |
| `truncate` | `(value: integer|number) -> {TYPE}` | Truncates into the valid range. |
| `clamp` | `(value: integer|number) -> {TYPE}` | Clamps into the valid range. |
| `is_valid` | `(value: integer|number) -> bool` | Checks whether the input fits without overflow. |
| `to_string` | `(value: {TYPE}) -> string` | Converts to a string. |

### Bit operations and endianness

| Method | Signature | Description |
| --- | --- | --- |
| `count_ones` | `(value: {TYPE}) -> u32` | Counts set bits. |
| `count_zeros` | `(value: {TYPE}) -> u32` | Counts unset bits. |
| `leading_zeros` | `(value: {TYPE}) -> u32` | Counts leading zero bits. |
| `trailing_zeros` | `(value: {TYPE}) -> u32` | Counts trailing zero bits. |
| `leading_ones` | `(value: {TYPE}) -> u32` | Counts leading one bits. |
| `trailing_ones` | `(value: {TYPE}) -> u32` | Counts trailing one bits. |
| `rotate_left` | `(value: {TYPE}, count: u32) -> {TYPE}` | Rotates left. |
| `rotate_right` | `(value: {TYPE}, count: u32) -> {TYPE}` | Rotates right. |
| `swap_bytes` | `(value: {TYPE}) -> {TYPE}` | Reverses byte order. |
| `reverse_bits` | `(value: {TYPE}) -> {TYPE}` | Reverses bit order. |
| `from_be` | `(value: {TYPE}) -> {TYPE}` | Converts big-endian to host order. |
| `from_le` | `(value: {TYPE}) -> {TYPE}` | Converts little-endian to host order. |
| `to_be` | `(value: {TYPE}) -> {TYPE}` | Converts host order to big-endian. |
| `to_le` | `(value: {TYPE}) -> {TYPE}` | Converts host order to little-endian. |

### Arithmetic strategies

The runtime provides several safe arithmetic families.

#### Checked arithmetic

Returns `nil` on overflow or divide-by-zero.

```lua
integer.{TYPE}.checked_add(lhs, rhs)
integer.{TYPE}.checked_sub(lhs, rhs)
integer.{TYPE}.checked_mul(lhs, rhs)
integer.{TYPE}.checked_div(lhs, rhs)
integer.{TYPE}.checked_rem(lhs, rhs)
integer.{TYPE}.checked_neg(value)
integer.{TYPE}.checked_shl(lhs, rhs)
integer.{TYPE}.checked_shr(lhs, rhs)
integer.{TYPE}.checked_pow(lhs, exp)
```

#### Saturating arithmetic

Clamps results to the representable bounds.

```lua
integer.{TYPE}.saturating_add(lhs, rhs)
integer.{TYPE}.saturating_sub(lhs, rhs)
integer.{TYPE}.saturating_mul(lhs, rhs)
integer.{TYPE}.saturating_div(lhs, rhs)
integer.{TYPE}.saturating_pow(lhs, exp)
```

#### Wrapping arithmetic

Wraps modulo the type range.

```lua
integer.{TYPE}.wrapping_add(lhs, rhs)
integer.{TYPE}.wrapping_sub(lhs, rhs)
integer.{TYPE}.wrapping_mul(lhs, rhs)
integer.{TYPE}.wrapping_div(lhs, rhs)
integer.{TYPE}.wrapping_rem(lhs, rhs)
integer.{TYPE}.wrapping_neg(value)
integer.{TYPE}.wrapping_shl(lhs, rhs)
integer.{TYPE}.wrapping_shr(lhs, rhs)
integer.{TYPE}.wrapping_pow(lhs, exp)
```

#### Overflowing arithmetic

Returns `(result, did_overflow)`.

```lua
integer.{TYPE}.overflowing_add(lhs, rhs)
integer.{TYPE}.overflowing_sub(lhs, rhs)
integer.{TYPE}.overflowing_mul(lhs, rhs)
integer.{TYPE}.overflowing_div(lhs, rhs)
integer.{TYPE}.overflowing_rem(lhs, rhs)
integer.{TYPE}.overflowing_neg(value)
integer.{TYPE}.overflowing_shl(lhs, rhs)
integer.{TYPE}.overflowing_shr(lhs, rhs)
integer.{TYPE}.overflowing_pow(lhs, exp)
```

#### Direct arithmetic and bitwise operations

These are the simple operations that may throw on invalid or overflowing inputs.

```lua
integer.{TYPE}.add(lhs, rhs)
integer.{TYPE}.sub(lhs, rhs)
integer.{TYPE}.mul(lhs, rhs)
integer.{TYPE}.div(lhs, rhs)
integer.{TYPE}.rem(lhs, rhs)
integer.{TYPE}.neg(value)
integer.{TYPE}.shl(lhs, rhs)
integer.{TYPE}.shr(lhs, rhs)
integer.{TYPE}.pow(lhs, exp)

integer.{TYPE}.bit_and(lhs, rhs)
integer.{TYPE}.bit_or(lhs, rhs)
integer.{TYPE}.bit_xor(lhs, rhs)
integer.{TYPE}.bit_not(value)
```

### Integer advice for AI authors

- Prefer checked or overflowing arithmetic when the code is handling user input or uncertain data.
- Do not assume signed and unsigned types behave identically.
- If a value must be kept within a known range, use `clamp()` or `saturating_*` functions.

---

## 7. Buffer API

The runtime includes a buffer system for binary manipulation.

### Factory functions

```lua
buffer.create(initial_capacity)
buffer.wrap(str)
```

### `Buffer` methods

| Method | Description |
| --- | --- |
| `:head()` / `:head(position)` | Reads or sets the current read position. |
| `:tail()` / `:tail(position)` | Reads or sets the current write position. |
| `:remaining()` | Returns the number of unread bytes. |
| `:capacity()` | Returns the total buffer capacity. |
| `:reset()` | Resets head and tail to zero. |
| `:reserve(length)` | Ensures enough capacity for future writes. |

### Buffer usage notes

- Buffers behave like FIFO streams of bytes.
- Reading and writing advances the internal head/tail positions.
- Use buffers when working with raw binary data, especially when parsing content or asset payloads.

### Example pattern

```lua
local buf = buffer.create(0)
buf:write_string("hello")
local data = buf:read_string(5)
print(data)
```

---

## 8. System utilities (`builtin`, `GuidHelper`, `hasher`)

### `builtin`

`builtin` is a placeholder namespace for built-in engine utilities and global primitives. It should be treated as a runtime-provided utility surface rather than a place for arbitrary custom logic.

### `GuidHelper`

Utilities for working with GUID-style values used by the engine. In this runtime build they are exposed through `game.guid`.

```lua
game.guid.from_content_hash(content_hash) -> Guid
game.guid.to_content_hash(guid) -> ContentHash
game.guid.hash(guid) -> u32
```

### `hasher`

Hash helpers for engine identifiers.

```lua
hasher.fnv1a32(value: string|Buffer) -> u32
hasher.crc32(value: string|Buffer) -> u32
hasher.crc64(value: string|Buffer) -> u64
```

### Utility guidance for AI authors

- If the script needs a GUID, prefer engine helpers instead of inventing custom formatting.
- If the task requires deterministic identifiers, use the provided hash helpers rather than relying on ad-hoc string hash logic.
- When testing runtime-exposed helpers, prefer the exact names verified in this build: `game.guid.hash`, `hasher.fnv1a32`, and `hasher.crc64`.

---

## 9. Common AI mistakes to avoid

These are the most common failure modes seen in EML mod generation.

1. Creating extra files instead of keeping the logic in `src/mod.lua`
2. Assuming that every exported asset is a full disk dump rather than a runtime snapshot
3. Using `io.export()` without checking `loader.features.export`
4. Assuming that `get_resources_by_type()` or `get_all_resources()` exposes every game asset on disk
5. Ignoring reflection and trying to interpret binary data without checking the type schema
6. Using raw numeric values without understanding the underlying `Type` or enum definition
7. Writing code that depends on unsupported external libraries or OS calls
8. Forgetting that the runtime is sandboxed and may fail silently or throw on unsupported operations10. Assuming that a type or helper name from one EML build will always exist in another build without checking the runtime surface
### Preferred behavior for mod authors

- Start from the runtime-visible state.
- Discover types and schemas with reflection.
- Use `pcall()` where appropriate.
- Emit structured, readable output.
- Keep everything self-contained and minimal.
- Prefer the runtime-tested patterns in this document over earlier generic assumptions.

---

## Practical example: safe export workflow

```lua
local function safe_export(path, content)
    if not (loader and loader.features and loader.features.export) then
        print("Export disabled")
        return false
    end

    local ok, err = pcall(function()
        io.export(path, content)
    end)

    if not ok then
        print("Export failed: " .. tostring(err))
        return false
    end

    return true
end

local resources = game.assets.get_resources_by_type("keen::ItemInfo") or {}
for _, res in ipairs(resources) do
    local label = res.guid or "unknown"
    print("resource", label)
end

safe_export("example_export.txt", "done")
```

---

## Final note

When generating EML code, the safest approach is to assume the runtime is constrained, the data is partially visible, and the script should be resilient, self-contained, and explicit. Use reflection when uncertain, avoid over-claiming completeness, and keep the implementation aligned with the documented EML runtime contract.
