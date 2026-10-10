# Blueprint panel height

Height of the World Editor window in logical pixels. Changes apply while the game is running.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("panelHeight", fallback)`: `panelHeight`.
- Type: `number`.
- Starting value: `190`.
- Number range: 140 to 400.
- The control is inferred from the value type.
- Minimum: `140`.
- Maximum: `400`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("panelHeight", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
