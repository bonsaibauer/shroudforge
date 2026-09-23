-- One entrypoint; capabilities select the phases in which it is called.
if runtime.phase == "pregame" then
    if loader.features.patch then
        -- Apply asset changes here.
    end
    return
end

return {
    on_load = function()
        runtime.require("runtime.lifecycle")
    end,
    on_update = function(delta_seconds)
        -- Live-world work here.
    end,
    on_unload = function()
        -- Release resources owned by this mod.
    end,
}
