# Read mod settings in Lua

The description and starting value live in `extended.mod.json`. Lua reads the current value with `shroudforge.settings.get(key, fallback)`.

```lua
local speed = shroudforge.settings.get("flightSpeed", 1.0)
```

The key must match exactly. The fallback should have the same type as `value`. Call `get` inside a callback when a player's change should take effect without restarting. A local value read once during loading does not update by itself.

A mod using this runtime function needs `runtime` in `mod.json`. The function does not save settings, Modloader does. See [Settings and controls](#doc-setting-controls).
