local operation = "runtime.patch.infinite_item_use"
local enabled = false

local function set_enabled(value)
    if enabled == value then return true end
    local ok, reason = runtime.patch.set_enabled(operation, value)
    if not ok then
        runtime.report_effect("write-failed", reason or "build-specific infinite-use patch failed")
        shroudforge.log.error("Infinite Item Use: " .. tostring(reason))
        return false
    end
    enabled = value
    runtime.report_effect("write-confirmed", value and "Build-verified native infinite-use patch enabled" or "Build-verified native infinite-use patch restored")
    return true
end

return {
    on_load = function()
        runtime.require("runtime.gameplay.patch")
        if not runtime.patch.available(operation) then
            error("Infinite Item Use patch is not validated for this game build")
        end
        if not set_enabled(true) then
            error("Infinite Item Use could not be enabled; see the runtime error log")
        end
    end,
    on_unload = function()
        if enabled then set_enabled(false) end
    end,
}
