# Tracking ID

Nonzero tracking value from the ItemInfo placement definition. The advanced native spawn and placement actions require it.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("trackingId", fallback)`: `trackingId`.
- Type: `number`.
- Starting value: `0`.
- Number range: 0 to no maximum.
- The control is inferred from the value type.
- Minimum: `0`.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("trackingId", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
