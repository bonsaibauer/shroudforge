local operation = "runtime.patch.no_fall_damage"
local enabled = false

local function set_enabled(value)
    local ok, reason = runtime.patch.set_enabled(operation, value)
    if not ok then
        runtime.report_effect("write-failed", reason or "build-specific fall-damage patch failed")
        shroudforge.log.error("No Fall Damage: " .. tostring(reason))
        return false
    end
    enabled = value
    runtime.report_effect("write-confirmed", value and "Build-verified native patch enabled" or "Build-verified native patch restored")
    return true
end

shroudforge.ui.on_action("toggleFallDamagePatch", function() set_enabled(not enabled) end)

return {
    on_load = function()
        runtime.require("runtime.gameplay.patch")
        if not runtime.patch.available(operation) then error("No Fall Damage patch is not validated for this game build") end
        if not set_enabled(true) then error("No Fall Damage could not be enabled; see the runtime error log") end
        shroudforge.log.info("No Fall Damage: build-specific native operation patched")
    end,
    on_unload = function() if enabled then set_enabled(false) end end,
}
