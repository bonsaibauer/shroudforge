# Maximum props per blueprint

Maximum props per blueprint. Capture, save, load, and paste reject larger blueprints instead of silently dropping props.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("maximumCopyableProps", fallback)`: `maximumCopyableProps`.
- Type: `number`.
- Starting value: `60000`.
- Number range: 1 to 1000000.
- The control is inferred from the value type.
- Minimum: `1`.
- Maximum: `1000000`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("maximumCopyableProps", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
