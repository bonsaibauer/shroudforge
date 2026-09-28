local operation = "runtime.patch.infinite_item_split"
local enabled = false

local function set_enabled(value)
    if enabled == value then return true end

    local ok, reason = runtime.patch.set_enabled(operation, value)
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
        if not runtime.patch.available(operation) then
            error("Infinite Item Split patch is not validated for this game build")
        end
        if not set_enabled(true) then
            error("Infinite Item Split could not enable its runtime patch; see the runtime error log")
        end
        shroudforge.log.info(
            "Infinite Item Split enabled the build-verified split-specific runtime patch"
        )
    end,
    on_unload = function()
        if enabled then set_enabled(false) end
    end,
}
