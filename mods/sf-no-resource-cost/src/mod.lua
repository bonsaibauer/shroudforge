local operation = "runtime.patch.no_resource_cost"
local enabled = false

local function set_enabled(value)
    if enabled == value then return true end
    local ok, reason = runtime.patch.set_enabled(operation, value)
    if not ok then
        runtime.report_effect("write-failed", reason or "build-specific recipe-cost patch failed")
        shroudforge.log.error("No Resource Cost: " .. tostring(reason))
        return false
    end
    enabled = value
    runtime.report_effect("write-confirmed", value and "Build-verified native recipe-cost patch enabled" or "Build-verified native recipe-cost patch restored")
    return true
end

return {
    on_load = function()
        runtime.require("runtime.gameplay.patch")
        if not runtime.patch.available(operation) then
            error("No Resource Cost patch is not validated for this game build")
        end
        if not set_enabled(true) then
            error("No Resource Cost could not be enabled; see the runtime error log")
        end
    end,
    on_unload = function()
        if enabled then set_enabled(false) end
    end,
}
