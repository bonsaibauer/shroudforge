# Keyboard shortcuts

This page shows and edits ShroudForge window shortcuts and mod shortcuts that a mod explicitly declares as `control: "keybind"` in its `extended.mod.json`.

## Assign a shortcut

Click the field and press the key or combination you want. Input uses physical SDL3 scancodes such as `SDL:64` for F7, `Ctrl+SDL:29`, or `Shift+SDL:100`. This keeps the physical key position consistent across keyboard layouts. SDL supplies readable labels for the current operating-system layout. Modifiers such as Ctrl, Alt, Shift, and Meta must match exactly.

While recording, ShroudForge blocks its window shortcuts and declared mod shortcuts across processes. Protection remains active after cancellation or a window focus change until all keys and mouse buttons are released, followed by three seconds without another key press or mouse click. New input restarts the waiting period after release. Escape cannot be recorded, with or without modifiers: the game uses it for its pause menu and to close its windows, so pressing it cancels the recording and keeps the previous shortcut.

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
local was_down = false

return {
    on_update = function()
        local down = shroudforge.input.is_keybind_down("pasteKey")
        if down and not was_down then
            -- Run the action.
        end
        was_down = down
    end,
}
```

`shroudforge.input.keybind_label("pasteKey")` returns the current readable SDL label for the operating-system layout. The key query works while a game window is focused. A declaration alone does not change a key hard-coded in a script. The mod must read this value through the API and use it for its action. Mod authors should trigger an action only on the transition from released to pressed so holding a key does not repeat the action on every update.

## Conflicts and other mods

ShroudForge reports conflicts among active ShroudForge window shortcuts and declared mod keybinds. Saving cannot introduce a new conflict between active shortcuts whose key and modifier combinations overlap.

Standalone EML mods and DLL mods that manage their own shortcuts cannot be reliably remapped or intercepted here. Set those shortcuts in the respective mod's settings or configuration if the mod provides that option. An EML mod can also appear here if it explicitly declares a matching `keybind` setting in `extended.mod.json` and uses the ShroudForge Lua API for it. This does not take over its independently managed EML or DLL shortcuts.
