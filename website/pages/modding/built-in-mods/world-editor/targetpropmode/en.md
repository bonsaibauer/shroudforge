# Props at target

Controls whether props at the target are kept or safely verified intersecting props are replaced.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("targetPropMode", fallback)`: `targetPropMode`.
- Type: `string`.
- Starting value: `"keep"`.
- Allowed values: `keep` for Keep existing props, `replace` for Replace intersecting props.
- The control is inferred from the value type.
- Options: `keep` (Keep existing props), `replace` (Replace intersecting props).

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("targetPropMode", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
