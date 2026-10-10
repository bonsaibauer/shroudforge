# Source cell Y (manual fallback)

Manual Y coordinate of the source cell when capturing without both cursor corners marked. Normally mark the selection with F5.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("sourceY", fallback)`: `sourceY`.
- Type: `number`.
- Starting value: `0`.
- Numeric value. The mod package declares no additional limit.
- The control is inferred from the value type.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("sourceY", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
