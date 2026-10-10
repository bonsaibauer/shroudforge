# Initial paste rotation (F3 cycles in game, 0–3)

Initial rotation of the active blueprint in quarter turns. 0, 1, 2, and 3 mean 0°, 90°, 180°, and 270°. F3 advances it in game.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("rotationQuarterTurns", fallback)`: `rotationQuarterTurns`.
- Type: `number`.
- Starting value: `0`.
- Number range: 0 to 3.
- The control is inferred from the value type.
- Minimum: `0`.
- Maximum: `3`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("rotationQuarterTurns", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
