--- @meta

--- Manages everything related to the runtime environment.
---
--- @class LoaderRuntime
local runtime = {}

--- NOTE: This function is currently not implemented and won't do anything.
---
--- Registers a dll file to be loaded by the mod loader when the game starts.
---
--- [loader.features.runtime.dll](lua://loader.features.runtime)
--- must be enabled for this function to work, otherwise it will throw an error.
---
--- # Errors
--- - If the file does not exist or cannot be read.
---
--- @param path string -- The path to the dll file to load, relative to the mod's root directory.
function runtime.register_dll(path) end
