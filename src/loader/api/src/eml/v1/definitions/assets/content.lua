---@meta

--- Represents a specific raw binary asset in the game, identified by its guid.
---
--- @class Content
--- @field guid Guid -- A 32 character hexadecimal string representing the asset's unique identifier.
--- @field size u32 -- The size of the asset in bytes.
--- @field hash0 u32 First word of the asset content hash.
--- @field hash1 u32 Second word of the asset content hash.
--- @field hash2 u32 Third word of the asset content hash.
local Content = {}

--- Reads the raw binary data of the asset.
---
--- @return Buffer -- A read-only buffer containing the raw binary data of the asset.
function Content:read_data() end
