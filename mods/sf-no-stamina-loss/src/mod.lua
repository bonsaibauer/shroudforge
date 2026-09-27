local operation = "runtime.patch.no_stamina_loss"
local enabled
local last_error

local function set_enabled(value)
    if enabled == value then return true end
    local ok, reason = runtime.patch.set_enabled(operation, value)
    if not ok then
        reason = reason or "build-specific stamina patch failed"
        if reason ~= last_error then
            runtime.report_effect("write-failed", reason)
            shroudforge.log.error("No Stamina Loss: " .. tostring(reason))
            last_error = reason
        end
        return false
    end
    enabled = value
    last_error = nil
    runtime.report_effect("write-confirmed", value and "Build-verified native stamina patch enabled" or "Build-verified native stamina patch restored")
    return true
end

local function apply_setting()
    local wanted = shroudforge.settings.get("preventDepletion") ~= false
    set_enabled(wanted)
end

return {
    update_interval_ms = 500,
    on_load = function()
        runtime.require("runtime.gameplay.patch")
        if not runtime.patch.available(operation) then
            error("No Stamina Loss patch is not validated for this game build")
        end
        if not set_enabled(shroudforge.settings.get("preventDepletion") ~= false) then
            error("No Stamina Loss could not apply its configured state; see the runtime error log")
        end
    end,
    on_update = apply_setting,
    on_unload = function() if enabled == true then set_enabled(false) end end,
}
