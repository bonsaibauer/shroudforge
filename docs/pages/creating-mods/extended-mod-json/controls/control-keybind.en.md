---
{
  "id": "control-keybind",
  "locale": "en",
  "title": "The `keybind` control",
  "summary": "For a keyboard or mouse shortcut stored as one setting value.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Controls",
      "Control types"
    ]
  }
}
---

# The `keybind` control

Declare a shortcut once as a `keybind` setting. The same setting appears in the mod settings and automatically under **Compatibility → Hotkeys**. Both views edit the same value.

```json
{
  "settings": {
    "pasteKey": {"value": "SDL:64", "label": "Paste blueprint", "control": "keybind", "applyAt": "live"}
  }
}
```

Read the declared setting in Lua. Do not repeat its default key in the script.

```lua
local was_down = false
return {
    on_update = function()
        local down = shroudforge.input.is_keybind_down("pasteKey")
        if down and not was_down then
            -- Call the same action that the mod's UI uses.
        end
        was_down = down
    end,
}
```

The shortcut field has two columns. Record the full keyboard combination on the left and an optional mouse combination on the right. Both alternatives belong to the same setting. Either one triggers the action.

While recording, ShroudForge blocks its window shortcuts and mod shortcuts queried through this API across processes. Protection remains active after cancellation or a window focus change until all keys and mouse buttons are released, followed by three seconds without another key press or mouse click. New input restarts the waiting period after release. Escape cannot be recorded, with or without modifiers: the game uses it for its pause menu and to close its windows, so pressing it cancels the recording and keeps the previous value.

The value remains one string. A keyboard chord stores an [SDL scancode](https://wiki.libsdl.org/SDL3/SDL_Scancode), for example `SDL:64` for F7, `Ctrl+SDL:29` for the Z position or `Shift+SDL:100` for the additional ISO key. A mouse chord stores an SDL button number, for example `Mouse:4` for X1 or `Ctrl+Mouse:4` with Ctrl. Join a keyboard and mouse chord with `|`, keyboard first, as in `Ctrl+SDL:29|Mouse:4`. At most one chord per device is allowed. Optional modifier prefixes occur once in `Ctrl`, `Alt`, `Shift`, `Meta` order and match exactly. An empty string disables the shortcut.

SDL supplies the displayed labels. `shroudforge.input.keybind_label("pasteKey")` returns the current label, including both alternatives when configured, or `nil` when inactive. Mouse observation uses SDL's global button state. Mouse-wheel shortcuts are not supported.

ShroudForge observes in-game shortcuts in a separate hidden process and sends input states and labels to the Lua runtime. It does not register SDL input inside the game process. Only the SDL binding strings described above are accepted. Windows virtual-key numbers, browser key names and the former raw `is_key_down` API are not supported.

Only declared `keybind` settings can use this API. Existing `enabledWhen` conditions also apply. Native third-party DLL shortcuts are not automatically controlled by this setting. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
