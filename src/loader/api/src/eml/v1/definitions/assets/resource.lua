---@meta

-- TODO: implement `Resource::original_data`

--- Represents a specific resource in the game, identified by its guid, type, and part.
---
--- @class Resource
--- @field guid Guid Unique identifier of this game resource.
--- @field type Type Reflected data layout used to interpret the resource data.
--- @field part u32 -- The part number of the asset, which is used for assets that are split into multiple parts.
--- @field data unknown -- The data of the asset whose structure is defined by the `type` field.
--- @field original_data unknown -- A read-only reference to the original data of the asset.
local Resource = {}
