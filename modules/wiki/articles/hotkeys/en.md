# Keyboard shortcuts

This page shows and edits ShroudForge window shortcuts and mod shortcuts that a mod explicitly declares as `control: "keybind"` in its `extended.mod.json`.

## Assign a shortcut

Click the field and press the key or combination you want. Input uses physical SDL3 scancodes such as `SDL:64` for F7, `Ctrl+SDL:29`, or `Shift+SDL:100`. This keeps the physical key position consistent across keyboard layouts. SDL supplies readable labels for the current operating-system layout. Modifiers such as Ctrl, Alt, Shift, and Meta must match exactly.

While recording, ShroudForge blocks its window shortcuts and declared mod shortcuts across processes. Protection remains active after cancellation or a window focus change until all keys and mouse buttons are released, followed by three seconds without another key press or mouse click. New input restarts the waiting period after release.

Use **Clear** to disable a shortcut. For ShroudForge windows, you can restore the built-in default. Mod shortcuts remain part of each mod's own settings. The fields in the mod settings and under **Compatibility → Hotkeys** edit the same saved value.

## Shortcuts in a ShroudForge mod

A mod author declares a shortcut as a setting in `extended.mod.json`:

```json
{
  "settings": {
    "pasteKey": {
      "value": "SDL:64",
      "label": "Paste blueprint",
      "control": "keybind",
      "applyAt": "live"
    }
  }
}
```

The mod reads this setting through the ShroudForge Lua API. It responds to the current value instead of hard-coding a key in the script:

```lua
return {
    on_update = function()
        if shroudforge.input.was_keybind_pressed("pasteKey") then
            -- Run the action.
        end
    end,
}
```

`shroudforge.input.was_keybind_pressed("pasteKey")` retains short taps between runtime updates and returns `true` once per mod and setting. Live saved changes reach the runtime within one second. Use `is_keybind_down` for actions that should repeat while held. The former `is_key_down("F1")` through `is_key_down("F8")` API has been unsupported since ShroudForge 1.7.6. Calling it logs a migration message and raises a Lua error. Declare a `keybind` setting instead and assign it in the Hotkey Manager under **Compatibility → Hotkeys**. `shroudforge.input.keybind_label("pasteKey")` returns the current readable SDL label for the operating-system layout. The key query works while a game window is focused. A declaration alone does not change a key hard-coded in a script. The mod must read this value through the API and use it for its action.

## Conflicts and other mods

ShroudForge reports conflicts among active ShroudForge window shortcuts and declared mod keybinds. Saving cannot introduce a new conflict between active shortcuts whose key and modifier combinations overlap.

Standalone EML mods and DLL mods that manage their own shortcuts cannot be reliably remapped or intercepted here. Set those shortcuts in the respective mod's settings or configuration if the mod provides that option. An EML mod can also appear here if it explicitly declares a matching `keybind` setting in `extended.mod.json` and uses the ShroudForge Lua API for it. This does not take over its independently managed EML or DLL shortcuts.
