# Modding

Read the main-menu state and selected local character name through the Lua runtime. Both queries use the same native data as the Modloader and require no world access.

## Prerequisites

Enable the mod and declare the `runtime` capability in `mod.json`:

```json
{
  "capabilities": ["runtime"]
}
```

For these client queries, set `targets` to `["client"]` in `extended.mod.json`. Call the functions from active runtime callbacks such as `on_load` and `on_update`. `runtime.phase == "ingame"` describes the started mod runtime and does not confirm entry into a world. Neither query needs an execution profile. Other world operations retain their own requirements.

## Detect the main menu

```lua
local in_menu, reason = runtime.is_main_menu()
```

| Return value | Meaning |
| --- | --- |
| `true, nil` | The main menu, including character, world and server selection, has been confirmed. |
| `false, nil` | A different original game state has been validated. |
| `nil, reason` | The state is unknown, changing, or the query is unavailable. |

Check `in_menu == true` or `in_menu == false` explicitly. `nil` is not a confirmed state. `false` alone does not confirm a readable world either. Check the operation you need separately, such as `runtime.has("runtime.ecs.read")`.

## Read the character name

```lua
local name, reason = runtime.player.name()
local display_name = name or "Flameborn"
```

`name, nil` returns the character name selected in the local main menu as a UTF-8 string. Unicode characters, as in `Ikiju 育樹`, are preserved. `nil, reason` means that no name is available yet, or access is unavailable or not allowed.

The mod chooses its own fallback name. The API inserts no default and does not use the Modloader's manual name selection. The value identifies no remote player and is neither a SteamID nor a Steam display name. A Dedicated Server has no local client character name.

## Shared cache

Lua and the Modloader read the same native cache. The existing runtime tick refreshes the menu state. The existing menu integration captures the name on the first available selection and again when selection or name metadata changes. Lua queries start no separate discovery and read no logs.

The last successfully captured name remains cached after joining a world. A cached name therefore does not confirm that the menu is open. `runtime.has("runtime.is_main_menu")` and `runtime.has("runtime.player.name")` report each query's availability. `runtime.status(...)` also supplies a reason.

## Example for `src/mod.lua`

This example reads the cache on load and every 500 ms afterwards. It chooses a fallback name and logs only changes:

```lua
runtime.require("runtime.lifecycle")

local previous_message = nil

local function refresh()
    local in_menu = runtime.is_main_menu()
    local name = runtime.player.name()
    local state = "unknown"
    if in_menu == true then
        state = "menu"
    elseif in_menu == false then
        state = "other"
    end

    local message = state .. ": " .. (name or "Flameborn")
    if message ~= previous_message then
        previous_message = message
        shroudforge.log.info(message)
    end
end

return {
    on_load = refresh,
    on_update = function()
        refresh()
    end,
    update_interval_ms = 500,
}
```
