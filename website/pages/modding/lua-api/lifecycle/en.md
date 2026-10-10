# Runtime callbacks

A runtime mod returns a table of callback functions. The loader calls them at the matching time. Declare `runtime` in `mod.json`.

```lua
runtime.require("runtime.lifecycle")

return {
  on_load = function()
    shroudforge.log.info("Mod runtime started")
  end,
  on_update = function(delta_seconds)
    -- Read current settings or process queued work here
  end,
  on_unload = function()
    shroudforge.log.info("Mod runtime stopped")
  end,
  update_interval_ms = 100
}
```

`on_load` runs when the mod runtime starts. `on_update` receives elapsed time in seconds. `on_unload` runs when the mod stops or unloads. The default update interval is 50 ms and the accepted range is 8 to 1000 ms. Missed intervals are not replayed.

Do not use `on_update` for unnecessary per-frame work. Read live settings again inside the callback. Check return values and status for network and native operations.
