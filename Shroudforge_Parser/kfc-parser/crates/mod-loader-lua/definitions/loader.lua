--- @meta

--- Provides information about the current mod loader environment.
---
--- @class Loader
--- @field is_client boolean -- Whether the current environment is the client.
--- @field is_server boolean -- Whether the current environment is the server.
--- @field features LoaderFeatures -- The features available in the current environment.
--- @field runtime LoaderRuntime -- Manages everything related to the runtime environment.
loader = {}

--- @class LoaderFeatures
---
--- Whether the patching feature is available.
--- When this is `true`, changes to game data will be applied to the game files.
--- @field patch boolean
---
--- Whether the import feature is available.
--- When this is `true`, mods can use `io.export` to export arbitrary data.
--- @field export boolean
---
--- All runtime-related features.
--- When this is `nil`, no runtime features are available.
--- @field runtime RuntimeFeatures?

--- @class RuntimeFeatures
---
--- When this is `true`, mods can use `loader.runtime.register_dll` to register dll files to be loaded by the mod loader when the game starts.
--- @field dll boolean

--- Returns true if the given mod is loaded.
---
--- @param mod_id string -- The id of the mod to check.
--- @return boolean -- True if the mod is loaded, false otherwise.
function loader.has_mod(mod_id) end
