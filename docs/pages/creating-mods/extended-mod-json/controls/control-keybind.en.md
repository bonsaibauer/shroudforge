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
return {
    on_update = function()
        if shroudforge.input.was_keybind_pressed("pasteKey") then
            -- Call the same action that the mod's UI uses.
        end
    end,
}
```

`was_keybind_pressed` retains a short tap even when it begins and ends between runtime updates. It returns `true` once per new press for each mod and setting. For actions that should repeat while held, use `is_keybind_down`.

The shortcut field has two columns. Record the full keyboard combination on the left and an optional mouse combination on the right. Both alternatives belong to the same setting. Either one triggers the action. Live saved changes reach the runtime on its next configuration refresh, within one second.

While recording, ShroudForge blocks its window shortcuts and mod shortcuts queried through this API across processes. Protection remains active after cancellation or a window focus change until no key or mouse button is held. A fixed three-second delay then runs, and new input does not reset it. Escape cannot be recorded, with or without modifiers: the game uses it for its pause menu and to close its windows, so pressing it cancels the recording and keeps the previous value. New F1 or F12 bindings require confirmation because Enshrouded or Steam may use them. Saving a binding that conflicts with another active ShroudForge shortcut is blocked.

The value remains one string. A keyboard chord stores an [SDL scancode](https://wiki.libsdl.org/SDL3/SDL_Scancode), for example `SDL:64` for F7, `Ctrl+SDL:29` for the Z position or `Shift+SDL:100` for the additional ISO key. A mouse chord stores an SDL button number, for example `Mouse:4` for X1 or `Ctrl+Mouse:4` with Ctrl. Join a keyboard and mouse chord with `|`, keyboard first, as in `Ctrl+SDL:29|Mouse:4`. At most one chord per device is allowed. Optional modifier prefixes occur once in `Ctrl`, `Alt`, `Shift`, `Meta` order and match exactly. An empty string disables the shortcut.

SDL supplies the displayed labels. `shroudforge.input.keybind_label("pasteKey")` returns the current label, including both alternatives when configured, or `nil` when inactive. Mouse observation uses SDL's global button state. Mouse-wheel shortcuts are not supported: a mod reads the wheel through [inputReservations](#doc-field-inputreservations) instead.

A shortcut may also be kept from the game while the mod needs it, for example while the hammer is in hand. The player chooses it with a padlock in front of the key. See [inputReservations](#doc-field-inputreservations).

ShroudForge observes in-game shortcuts in a separate hidden process and sends input states and labels to the Lua runtime. It does not register SDL input inside the game process. Only the SDL binding strings described above are accepted. The former `is_key_down("F1")` through `is_key_down("F8")` API has been unsupported since ShroudForge 1.7.6. Calling it logs a migration message and raises a Lua error. Declare a `keybind` setting instead and assign it in the Hotkey Manager under **Compatibility → Hotkeys**.

Only declared `keybind` settings can use this API. Existing `enabledWhen` conditions also apply. Native third-party DLL shortcuts are not automatically controlled by this setting. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
