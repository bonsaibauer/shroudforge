# Panel position Y (from game top)

Vertical offset of the World Editor window from the top of the game window.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("panelPositionY", fallback)`: `panelPositionY`.
- Type: `number`.
- Starting value: `92`.
- Number range: 0 to 8192.
- The control is inferred from the value type.
- Minimum: `0`.
- Maximum: `8192`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("panelPositionY", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
