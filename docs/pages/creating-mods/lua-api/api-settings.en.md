---
{
  "id": "api-settings",
  "locale": "en",
  "title": "Read mod settings in Lua",
  "summary": "The key in extended.mod.json is the same key your Lua code passes to shroudforge.settings.get.",
  "navigation": {
    "path": [
      "Lua API guides",
      "Feature guides"
    ]
  },
  "apiSymbols": [
    "shroudforge.settings.get",
    "shroudforge.settings"
  ]
}
---

# Read mod settings in Lua

The description and starting value live in `extended.mod.json`. SF Survival declares the setting like this:

```json
{
  "settings": {
    "infiniteStamina": {
      "value": true,
      "label": "Infinite Stamina",
      "description": "Keeps stamina full and adjusts automatically when its maximum changes."
    }
  }
}
```

Lua reads the current value with `shroudforge.settings.get(key, fallback)`.

```lua
local infiniteStamina = shroudforge.settings.get("infiniteStamina", false)
```

This example comes from `mods/sf-survival/extended.mod.json`, where `infiniteStamina` has the Boolean package default `true`. When the saved value is `true`, this call returns the Lua Boolean `true`. When the saved value is `false`, it returns `false`. If no saved value exists, it returns the `false` fallback. Without a fallback, a missing value returns `nil`. The function returns the setting value directly, not inside a result object.

The key must match exactly. The fallback should have the same type as `value`. Call `get` inside a callback when a player's change should take effect without restarting. A local value read once during loading does not update by itself.

A mod using this runtime function needs `runtime` in `mod.json`. The function does not save settings, Modloader does. See [Settings and controls](#doc-setting-controls).
