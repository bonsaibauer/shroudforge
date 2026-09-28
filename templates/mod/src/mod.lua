runtime.require("runtime.lifecycle")

local function setting(name, fallback)
    return shroudforge.settings.get(name, fallback)
end

local function log_greeting()
    local greeting = tostring(setting("greeting", "Hello from the ShroudForge runtime!"))
    local severity = setting("severity", "info")

    if severity == "warn" then
        shroudforge.log.warn(greeting)
    else
        shroudforge.log.info(greeting)
    end
end

shroudforge.ui.on_action("logGreeting", log_greeting)

return {
    on_load = function()
        if setting("showOnLoad", true) then
            log_greeting()
        end
    end,
}
