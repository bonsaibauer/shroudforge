return {
    on_load = function()
        shroudforge.log.info("Loaded on " .. game.version)
        runtime.require("runtime.lifecycle")
    end,

    on_update = function(delta_seconds)
        -- In-game work belongs to runtime.*; native game metadata belongs to game.*.
    end,

    on_unload = function()
        shroudforge.log.info("Unloaded")
    end,
}
