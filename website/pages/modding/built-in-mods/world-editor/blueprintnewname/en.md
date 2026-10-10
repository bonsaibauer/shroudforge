# New name for rename or duplicate

Destination name for the mod actions that rename or duplicate a blueprint.

## Meaning and default

Copy, rotate, save, and paste parts of the world.

- Mod: World Editor (`world-editor`).
- Key read with `shroudforge.settings.get("blueprintNewName", fallback)`: `blueprintNewName`.
- Type: `string`.
- Starting value: `"my_blueprint_copy"`.
- Text length: 1 to 64 characters.
- The control is inferred from the value type.

## Source of this information

This page is based on `mods/world-editor/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("blueprintNewName", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
