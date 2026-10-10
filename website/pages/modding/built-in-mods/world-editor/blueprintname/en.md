# Persistent blueprint name

Default name for saving a blueprint and the first name used by rename or duplicate actions.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("blueprintName", fallback)`: `blueprintName`.
- Type: `string`.
- Starting value: `"my_blueprint"`.
- Text length: 1 to 64 characters.
- The control is inferred from the value type.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("blueprintName", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
