# Auto Refill

Turns repeated stamina refill to maximum on or off.

## Meaning and default

Repeatedly refills stamina to maximum.

- Mod: SF Auto Stamina Refill (`sf-auto-stamina-refill`).
- Key read with `shroudforge.settings.get("autoRefill", fallback)`: `autoRefill`.
- Type: `boolean`.
- Starting value: `true`.
- Allowed values: `true` or `false`.
- The control is inferred from the value type.

## Source of this information

This page is based on `mods/sf-auto-stamina-refill/extended.mod.json`. Lua reads the current value with `shroudforge.settings.get("autoRefill", fallback)`. Modloader saves player changes in the extension file. [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/main/mods/sf-auto-stamina-refill/src/mod.lua).
