# Size Y (manual fallback)

Manual capture extent along Y in voxel cells. Used only when there is no complete selection marked by cursor corners.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("sizeY", fallback)`: `sizeY`.
- Type: `number`.
- Starting value: `8`.
- Number range: 1 to 65536.
- The control is inferred from the value type.
- Minimum: `1`.
- Maximum: `65536`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("sizeY", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
