# Panel position X (−1 = centered)

Horizontal position relative to the game window. A value of −1 centers the window, nonnegative values set its left offset.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("panelPositionX", fallback)`: `panelPositionX`.
- Type: `number`.
- Starting value: `-1`.
- Number range: -1 to 8192.
- The control is inferred from the value type.
- Minimum: `-1`.
- Maximum: `8192`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("panelPositionX", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
