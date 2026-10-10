# Voxel mode

Controls how voxels are combined during paste. `replace` replaces target cells, while `add` adds only occupied source cells.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("pasteVoxelMode", fallback)`: `pasteVoxelMode`.
- Type: `string`.
- Starting value: `"replace"`.
- Allowed values: `replace` for Replace target voxels, `add` for Add occupied source voxels.
- The control is inferred from the value type.
- Options: `replace` (Replace target voxels), `add` (Add occupied source voxels).

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("pasteVoxelMode", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
