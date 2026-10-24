---
{
  "id": "modding-main-menu",
  "locale": "en",
  "title": "Query the main-menu state with Lua",
  "summary": "runtime.is_main_menu returns the cached main-menu state without requiring world access. Unknown states remain explicitly unknown.",
  "navigation": {
    "path": ["Modding"],
    "label": "Detect the main menu"
  },
  "apiSymbols": ["runtime.is_main_menu"]
}
---

# Query the main-menu state with Lua

`runtime.is_main_menu()` reports whether the local game client is in the main menu. This includes character, world, and server selection screens. A mod can use it to adapt its own display or work to time spent in the menu.

## Prerequisites

Declare the [`runtime` capability](#doc-capability-runtime) in `mod.json` and enable the mod in the Modloader:

```json
{
  "capabilities": ["runtime"]
}
```

The example runs on the client. Set [targets](#doc-field-targets) to `["client"]` in `extended.mod.json`. The query works in [runtime callbacks](#doc-api-lifecycle), including before entry into a world. It requires no world access or execution profile.

## Call and return values

The function takes no arguments:

```lua
local in_menu, reason = runtime.is_main_menu()
```

| Result | Meaning |
| --- | --- |
| `true, nil` | The original game state confirms the main menu and its selection screens. |
| `false, nil` | A different original game state has been validated. |
| `nil, reason` | The state is unknown, changing, or the query is unavailable. |

Check `in_menu == true` or `in_menu == false`. `nil` confirms neither menu nor world. `false` alone does not confirm a readable ECS world. Check the operation you need separately, such as `runtime.has("runtime.ecs.read")`.

`runtime.has("runtime.is_main_menu")` and `runtime.status("runtime.is_main_menu")` describe the query's current availability. An available query can return `false`.

## Complete Lua example

This `src/mod.lua` reads the cache on load and every 250 ms afterwards. It reports only changes:

```lua
runtime.require("runtime.lifecycle")

local previous_state = nil

local function update_menu_state()
  local in_menu = runtime.is_main_menu()
  local state = "unknown"
  if in_menu == true then
    state = "menu"
  elseif in_menu == false then
    state = "other"
  end

  if state ~= previous_state then
    previous_state = state
    shroudforge.log.info("Menu state: " .. state)
  end
end

return {
  on_load = update_menu_state,
  on_update = function(delta_seconds)
    update_menu_state()
  end,
  update_interval_ms = 250
}
```

## Shared source and limits

The function reads the same native cache as the Modloader UI. The existing runtime tick refreshes menu detection. The Lua call starts no additional discovery and reads no logs.

Desktop mode and the confirmed main menu do not require world access. The Modloader therefore shows enabled, error-free mods as **Ready** there. Mod errors, blocks, and required restarts retain their own status. A Dedicated Server has no local client main menu.

Read the local character name with [`runtime.player.name()`](#doc-modding-player-name). Function contracts are listed in the [API reference](#api).
