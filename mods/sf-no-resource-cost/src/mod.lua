local modifier_id = "zero_resource_argument"
local modifier
local enabled = false

local function set_enabled(value)
    if enabled == value then return true end
    local ok, reason = modifier.set_enabled(value)
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
        local reason
        modifier, reason = runtime.functions.bind_modifier(modifier_id)
        if not modifier or not modifier.available() then
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
