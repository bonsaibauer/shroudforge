local modifier_id = "refill_stamina"
local modifier
local enabled
local last_error

local function set_enabled(value)
    if enabled == value then return true end
    local ok, reason = modifier.set_enabled(value)
    if not ok then
        reason = reason or "build-specific stamina patch failed"
        if reason ~= last_error then
            runtime.report_effect("write-failed", reason)
            shroudforge.log.error("Auto Stamina Refill: " .. tostring(reason))
            last_error = reason
        end
        return false
    end
    enabled = value
    last_error = nil
    runtime.report_effect("write-confirmed", value and "Build-verified Stamina = Stamina_Max modifier enabled" or "Build-verified Stamina = Stamina_Max modifier restored")
    return true
end

local function apply_setting()
    local wanted = shroudforge.settings.get("autoRefill") ~= false
    set_enabled(wanted)
end

return {
    update_interval_ms = 500,
    on_load = function()
        runtime.require("runtime.gameplay.patch")
        local reason
        modifier, reason = runtime.functions.bind_modifier(modifier_id)
        if not modifier or not modifier.available() then
            error("Auto Stamina Refill patch is not validated for this game build")
        end
        if not set_enabled(shroudforge.settings.get("autoRefill") ~= false) then
            error("Auto Stamina Refill could not apply its configured state; see the runtime error log")
        end
    end,
    on_update = apply_setting,
    on_unload = function() if enabled == true then set_enabled(false) end end,
}
