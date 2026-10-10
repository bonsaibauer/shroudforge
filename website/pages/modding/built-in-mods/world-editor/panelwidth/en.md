# Blueprint panel width

Width of the World Editor window in logical pixels. Changes apply while the game is running.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("panelWidth", fallback)`: `panelWidth`.
- Type: `number`.
- Starting value: `1180`.
- Number range: 760 to 1920.
- The control is inferred from the value type.
- Minimum: `760`.
- Maximum: `1920`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("panelWidth", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
