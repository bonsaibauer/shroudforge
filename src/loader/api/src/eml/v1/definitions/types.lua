---@meta

--- Read-only view of data types reflected from the active Enshrouded build.
--- These descriptions report layouts and do not make game memory safe to change.

---@class TypeRegistry
---@field count integer All reflected types, including DS layouts.
---@field version string Current KFC data version; not an executable fingerprint.
local TypeRegistry = {}

--- Native storage descriptions, not Lua-owned or automatically writable containers.
---@class NativeDsString
---@class NativeDsArray<T>
---@class NativeDsOptional<T>
---@class NativeDsVariant<T>

---@alias TypeSelector string|Type

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

--- Returns every reflected type that matches the supplied filters.
---@param options {prefix:string?,attribute:string?,primitive_type:PrimitiveType?,has_ds:boolean?}? Optional filters for name prefix, attribute, primitive type, and DS storage.
---@return Type[]
function TypeRegistry.find(options) end

--- Looks up one type by its exact qualified name.
--- @param qualified_name string Full reflected name, for example `keen::SomeType`.
--- @return Type
function TypeRegistry.get(qualified_name) end

--- Alias for get that makes the exact qualified-name lookup explicit.
--- @param qualified_name string Full reflected type name.
--- @return Type
function TypeRegistry.get_by_qualified_name(qualified_name) end

--- Looks up one type by its Impact name.
--- @param impact_name string Name used by the game's Impact data.
--- @return Type
function TypeRegistry.get_by_impact_name(impact_name) end

--- Includes every reflected type by default. Pass false for the historical non-DS view.
--- @param include_ds boolean? Default true.
--- @return Type[]
function TypeRegistry.get_all(include_ds) end

--- Returns the reflected type of a supported Lua value, or nil when the value has no game type.
--- @param value any Lua value to inspect.
--- @return Type
function TypeRegistry.of(value) end

--- One reflected game type with its name, storage layout, fields, and metadata.
--- @class Type
---
--- @field index integer Zero-based reflection index, not an ECS component ID.
--- @field name string Short type name.
--- @field impact_name string Name used by the game's Impact data.
--- @field qualified_name string Full reflected name including its namespace.
--- @field namespace string[] Namespace segments from outermost to innermost.
--- @field inner_type Type? Element type for arrays or wrapped types, when available.
--- @field size u32 Size of the reflected value in bytes.
--- @field alignment u32 Required byte alignment of the reflected value.
--- @field element_alignment u32 Required byte alignment of array elements.
--- @field field_count u32 Number of fields exposed by this type.
--- @field primitive_type PrimitiveType Storage category recorded in the KFC data.
--- @field flags TypeFlag[] Decoded layout flags recorded for this type.
--- @field flags_bits integer Original bitset.
--- @field has_ds boolean True when the layout contains DS storage.
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

--- A structure field with its effective inherited layout information.
---@class ResolvedStructField: StructField
---@field declaring_type Type Type that originally declared this field.

--- A field entry in a reflected structure definition.
--- @class StructField
--- @field name string Field name in the reflected layout.
--- @field type Type Reflected type of the field value.
--- @field data_offset u32 Byte offset of the field within its containing value.
--- @field attributes table<string, Attribute> Metadata attributes attached to the field.

--- One named value in a reflected enum.
--- @class EnumField
--- @field name string Name of this enum variant.
--- @field value u64 Numeric value of this enum variant.
--- @field value_hex string Exact unsigned 64-bit representation.

--- Metadata attribute attached to a reflected type or field.
--- @class Attribute
--- @field name string Attribute name stored in the KFC metadata.
--- @field namespace string[] Namespace segments that own the attribute.
--- @field type Type? Reflected value type when one is known.
--- @field value string Raw textual attribute value from the KFC metadata.

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
