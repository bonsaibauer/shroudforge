# Blueprint up axis (stored on capture)

Up axis for new captures. It is stored in the blueprint, and existing blueprints keep their own axis.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("rotationAxis", fallback)`: `rotationAxis`.
- Type: `string`.
- Starting value: `"y"`.
- Allowed values: `x` for X axis, `y` for Y axis, `z` for Z axis.
- The control is inferred from the value type.
- Options: `x` (X axis), `y` (Y axis), `z` (Z axis).

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("rotationAxis", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
