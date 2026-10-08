---@meta

--- TODO: add documentation

---@class TypeRegistry
---@field count integer All reflected types, including DS layouts.
---@field version string Current KFC data version; not an executable fingerprint.
local TypeRegistry = {}

--- Native storage descriptions, not Lua-owned or automatically writable containers.
---@class NativeDsString
---@class NativeDsArray<T>
---@class NativeDsOptional<T>
---@class NativeDsVariant<T>

---@alias TypeSelector string|integer|Type|{hash:integer,domain:'qualified'|'internal'|'name'|'impact'}

--- Resolve against the current process registry. Ambiguous hashes return nil and a reason.
---@param selector TypeSelector
---@return Type? type
---@return string? reason
function TypeRegistry.resolve(selector) end

--- Registry indices are zero-based and valid only within this registry/build.
---@param index integer
---@return Type?
function TypeRegistry.get_by_index(index) end

--- Return all collisions instead of selecting an arbitrary matching type.
---@param hash integer
---@param domain 'qualified'|'internal'|'name'|'impact'?
---@return Type[]
function TypeRegistry.find_by_hash(hash, domain) end

---@param options {prefix:string?,attribute:string?,primitive_type:PrimitiveType?,has_ds:boolean?}?
---@return Type[]
function TypeRegistry.find(options) end

--- @param qualified_name string
--- @return Type
function TypeRegistry.get(qualified_name) end

--- @param qualified_name string
--- @return Type
function TypeRegistry.get_by_qualified_name(qualified_name) end

--- @param impact_name string
--- @return Type
function TypeRegistry.get_by_impact_name(impact_name) end

--- Resolve an unsigned 32-bit qualified hash in the current executable's registry.
--- @param hash integer
--- @return Type? type
--- @return string? reason Missing and ambiguous hashes return nil with a reason.
function TypeRegistry.get_by_qualified_hash(hash) end

--- Internal hashes describe layouts and can be shared by several types.
--- @param hash integer
--- @return Type? type
--- @return string? reason Ambiguous hashes require a qualified name/hash.
function TypeRegistry.get_by_internal_hash(hash) end

--- Includes every reflected type by default. Pass false for the historical non-DS view.
--- @param include_ds boolean? Default true.
--- @return Type[]
function TypeRegistry.get_all(include_ds) end

--- @param value any
--- @return Type
function TypeRegistry.of(value) end

--- @class Type
---
--- @field index integer Zero-based reflection index, not an ECS component ID.
--- @field name string
--- @field impact_name string
--- @field qualified_name string
--- @field qualified_hash integer
--- @field internal_hash integer
--- @field name_hash integer
--- @field impact_hash integer
--- @field namespace string[]
--- @field inner_type Type?
--- @field size u32
--- @field alignment u32
--- @field element_alignment u32
--- @field field_count u32
--- @field primitive_type PrimitiveType
--- @field flags TypeFlag[]
--- @field flags_bits integer Original bitset.
--- @field has_ds boolean The layout contains DS storage.
---
--- @field struct_fields table<string, StructField>
--- @field fields ResolvedStructField[] Base-first effective fields, with inherited offsets and declaring type.
--- @field enum_fields table<string, EnumField>
--- @field attributes table<string, Attribute>
---
--- @field default_bytes string? Exact reflected bytes, including embedded zero bytes. Use runtime.values.new for a decoded value.
local Type = {}

--- Checks struct/typedef inheritance; an array is not a subtype of its element.
---@param parent TypeSelector
---@return boolean
function Type:is_a(parent) end

---@class ResolvedStructField: StructField
---@field declaring_type Type

--- @class StructField
--- @field name string
--- @field type Type
--- @field data_offset u32
--- @field attributes table<string, Attribute>

--- @class EnumField
--- @field name string
--- @field value u64
--- @field value_hex string Exact unsigned 64-bit representation.

--- @class Attribute
--- @field name string
--- @field namespace string[]
--- @field type Type?
--- @field value string

---@alias PrimitiveType
---| "None"
---| "Bool"
---| "UInt8"
---| "SInt8"
---| "UInt16"
---| "SInt16"
---| "UInt32"
---| "SInt32"
---| "UInt64"
---| "SInt64"
---| "Float32"
---| "Float64"
---| "Enum"
---| "Bitmask8"
---| "Bitmask16"
---| "Bitmask32"
---| "Bitmask64"
---| "Typedef"
---| "Struct"
---| "StaticArray"
---| "DsArray"
---| "DsString"
---| "DsOptional"
---| "DsVariant"
---| "BlobArray"
---| "BlobString"
---| "BlobOptional"
---| "BlobVariant"
---| "ObjectReference"
---| "Guid"

---@alias TypeFlag
---| "HAS_DS"
---| "HAS_BLOB_ARRAY"
---| "HAS_BLOB_STRING"
---| "HAS_BLOB_OPTIONAL"
---| "HAS_BLOB_VARIANT"
---| "IS_GPU_UNIFORM"
---| "IS_GPU_STORAGE"
---| "IS_GPU_CONSTANT"
