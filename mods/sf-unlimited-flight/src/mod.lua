local modifier_id = "override_rotation_constant"
local modifier
local enabled = false

local function set_enabled(value)
    local ok, reason = modifier.set_enabled(value)
    if not ok then
        runtime.report_effect("write-failed", reason or "build-specific flight patch failed")
        shroudforge.log.error("Unlimited Flight: " .. tostring(reason))
        return false
    end
    enabled = value
    runtime.report_effect("write-confirmed", value and "Build-verified actor_rotation modifier enabled" or "Build-verified native patch restored")
    return true
end

shroudforge.ui.on_action("toggleFlight", function() set_enabled(not enabled) end)

return {
    on_load = function()
        runtime.require("runtime.gameplay.patch")
        local reason
        modifier, reason = runtime.functions.bind_modifier(modifier_id)
        if not modifier or not modifier.available() then error("Unlimited Flight patch is not validated for this game build") end
        if not set_enabled(true) then error("Unlimited Flight could not be enabled. See the runtime error log.") end
    end,
    on_unload = function() if enabled then set_enabled(false) end end,
}
