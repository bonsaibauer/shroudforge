local modifier_id = "preserve_split_source"
local modifier
local enabled = false

local function set_enabled(value)
    if enabled == value then return true end

    local ok, reason = modifier.set_enabled(value)
    if not ok then
        runtime.report_effect("write-failed", reason or "build-specific item-split patch failed")
        shroudforge.log.error("Infinite Item Split: " .. tostring(reason))
        return false
    end

    enabled = value
    runtime.report_effect(
        "write-confirmed",
        value and "Build-verified item-split patch enabled" or
            "Build-verified item-split patch restored"
    )
    return true
end

return {
    on_load = function()
        runtime.require("runtime.gameplay.patch")
        local reason
        modifier, reason = runtime.functions.bind_modifier(modifier_id)
        if not modifier or not modifier.available() then
            error("Infinite Item Split patch is not validated for this game build")
        end
        if not set_enabled(true) then
            error("Infinite Item Split could not enable its function modifier. See the runtime error log.")
        end
        shroudforge.log.info(
            "Infinite Item Split enabled a shared inventory subtraction modifier in this process. Other inventory callers are also affected."
        )
    end,
    on_unload = function()
        if enabled then set_enabled(false) end
    end,
}
