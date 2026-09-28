--- @meta

--- @class EmlLoaderFeatures
--- @field patch boolean Whether this mod may write game assets during startup preparation.
--- @field export boolean Whether this mod may write files through io.export.
--- @field runtime EmlRuntimeFeatures Runtime functions available in the running game process.

--- @class EmlRuntimeFeatures
--- @field dll boolean Whether EML package-local DLLs can be loaded in the current game process.

--- @class EmlLoader
--- @field is_client boolean
--- @field is_server boolean
--- @field features EmlLoaderFeatures
--- @field runtime EmlLoaderRuntime
loader = {}

--- Returns whether a mod with the given ID is present in the current registry.
--- @param mod_id string
--- @return boolean
function loader.has_mod(mod_id) end

--- @class EmlLoaderRuntime
--- @field register_dll fun(path: string) Load a DLL relative to the mod package root for the lifetime of the game process.
