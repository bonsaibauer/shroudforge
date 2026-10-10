# SF Production Time

Sets timed production recipes to a chosen base duration. World speed settings still apply.

## Package details

- Mod ID: `sf-production-time`
- Target processes: client and Dedicated Server
- Runtime capability: no
- Declared player settings: 1

## Settings

## Production time

Apply the same value to client and host/server before starting the game. World speed settings multiply this base duration. Instant recipes and inventory quantities are preserved.

- [Base production time (seconds)](#doc-builtin-sf-production-time-seconds)

Settings are stored in the mod package's `extended.mod.json`. The mod reads them with `shroudforge.settings.get(key, fallback)`. See the [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/sf-production-time/src/mod.lua) for implementation details.
