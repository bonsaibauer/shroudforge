---
{
  "id": "modding-player-name",
  "locale": "en",
  "title": "Query the local character name with Lua",
  "summary": "runtime.player.name reads the selected character name from the shared native menu cache. The mod chooses its own fallback name.",
  "navigation": {
    "path": ["Modding"],
    "label": "Read the character name"
  },
  "apiSymbols": ["runtime.player.name", "runtime.player"],
  "apiPrefixes": ["RuntimePlayerApi."]
}
---

# Query the local character name with Lua

`runtime.player.name()` returns the name selected in the local game client's main menu. Unicode characters are preserved. Use it for a personal mod display or a notification, for example.

## Prerequisites

Declare the [`runtime` capability](#doc-capability-runtime) in `mod.json`, enable the mod, and use a [runtime callback](#doc-api-lifecycle):

```json
{
  "capabilities": ["runtime"]
}
```

For this client example, set [targets](#doc-field-targets) to `["client"]` in `extended.mod.json`. The name can already be available in the main menu. World access is not required.

## Call and return values

The function takes no arguments:

```lua
local name, reason = runtime.player.name()
local display_name = name or "Flameborn"
```

| Result | Meaning |
| --- | --- |
| `name, nil` | The selected local character name is cached as a Lua string. |
| `nil, reason` | No name has been captured yet, the native query is unavailable, or the mod is not allowed to access it. |

The API does not insert a fallback name. The example chooses the mod's default with `name or "Flameborn"`. The Modloader's name setting is independent of that choice.

`runtime.has("runtime.player.name")` reports whether a name is currently available. `runtime.status("runtime.player.name")` also supplies a reason when the query is unavailable.

## Complete Lua example with a later retry

This `src/mod.lua` reads the name on load and uses the default until a name is available. The existing update callback reads the cache again every 500 ms and picks up a name that becomes available or changes later. It reports only changes:

```lua
runtime.require("runtime.lifecycle")

local display_name = nil

local function update_player_name()
  local name = runtime.player.name()
  local current_name = name or "Flameborn"

  if current_name ~= display_name then
    display_name = current_name
    shroudforge.log.info("Character name: " .. display_name)
  end
end

return {
  on_load = update_player_name,
  on_update = function(delta_seconds)
    update_player_name()
  end,
  update_interval_ms = 500
}
```

## Cache and name changes

The native menu binding captures the name on the first available selection and again when selection or name metadata changes. Lua and the Modloader use the same cache. A Lua call starts no log parsing or new game discovery.

The last successfully captured name remains cached after joining a world. It is the locally selected character name. It does not identify a remote player and is neither a SteamID nor a Steam display name. The function supplies no local character name on a Dedicated Server.

Use [`runtime.is_main_menu()`](#doc-modding-main-menu) to query whether the client is currently in the main menu. A cached name alone does not confirm that the menu is open.
