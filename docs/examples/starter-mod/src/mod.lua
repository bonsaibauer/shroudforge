runtime.require("runtime.lifecycle")

local locale = shroudforge.locale()

local function text(key, fallback)
    return shroudforge.i18n.get(key, locale, fallback)
end

local function setting(name, fallback)
    return shroudforge.settings.get(name, fallback)
end

local function log_greeting()
    local greeting = tostring(setting("greeting", "Hello from the ShroudForge runtime!"))
    local severity = setting("severity", "info")
    local message = text("log.greeting_prefix", "Greeting") .. ": " .. greeting

    if severity == "warn" then
        shroudforge.log.warn(message)
    else
        shroudforge.log.info(message)
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
