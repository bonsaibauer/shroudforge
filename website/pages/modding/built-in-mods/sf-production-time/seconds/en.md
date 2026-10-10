# Base production time (seconds)

Sets the base duration in seconds for timed production recipes. Set the same value on the client and host or server before starting the game.

## Meaning and default

Sets timed production recipes to a chosen base duration. World speed settings still apply.

- Mod: SF Production Time (`sf-production-time`).
- Key read with `shroudforge.settings.get("seconds", fallback)`: `seconds`.
- Type: `number`.
- Starting value: `1`.
- Number range: 0.1 to 3600.
- The control is inferred from the value type.
- Minimum: `0.1`.
- Maximum: `3600`.

## Source of this information

This page is based on `mods/sf-production-time/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("seconds", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/sf-production-time/src/mod.lua).

[Mod guide](https://github.com/bonsaibauer/shroudforge/blob/main/mods/sf-production-time/README.md)
