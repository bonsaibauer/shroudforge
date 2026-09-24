# Bundled Mods

## Purpose

This directory contains the Lua mods shipped with ShroudForge releases. Every child directory is an independent mod package with a `mod.json` manifest and a `src/mod.lua` entrypoint.

## Current mods

| Mod | Execution | Runtime scope | Behavior | Modloader UI |
| --- | --- | --- | --- | --- |
| Flight | Runtime | Client | Keeps local locomotion in the flying state and can suppress fall damage | Settings and reset action |
| Infinite Item Split | Runtime | Client | Restores the amount removed from a source stack after a split | Split-type selection |
| Infinite Item Use | Runtime | Client | Restores a consumed item to its addressed inventory slot | Item ID exclusions |
| No Fall Damage | Runtime | Client | Clears the local player's typed fall-damage state | Manual reset action |
| No Resource Cost | Startup assets | Client and server | Sets selected recipe input and resource costs to zero | Item, energy, and water controls |
| No Stamina Loss | Runtime | Client | Clears depletion and optionally restores typed maximum stamina | Depletion and refill controls |
| Unlock Blueprints | Startup assets | Client and server | Replaces supported recipe knowledge requirements | Lifecycle information and restart notice |

## Execution and API policy

The loader derives execution from the Lua entrypoint's ShroudForge API use. `runtime.require("game.assets.write")` and calls under `game.assets` select startup asset preparation. `runtime.require("runtime.lifecycle")`, lifecycle callbacks, and `runtime.ecs` select the in-game runtime. `io.export` selects export support. Runtime ECS currently targets the Client process; asset and export work can run in Client or Server processes. The loader shows only Client, Server, or Client + Server.

Authors do not add `capabilities`, `target`, API ranges, or per-setting apply-phase fields to `mod.json`. Runtime-only mods can be enabled, disabled, and reconfigured live through `on_load`, `on_unload`, and `shroudforge.settings`. Read live settings inside lifecycle callbacks rather than caching them at module initialization. Asset-writing changes take effect on the next game start. A package that uses both runtime and asset APIs is conservatively treated as next-start for activation and settings.

Keep the Lua entrypoint's top level side-effect free. The loader initializes disabled runtime modules so their lifecycle can be activated without restarting, but it denies runtime ECS operations until `on_load` begins. Put gameplay reads and writes inside `on_load`/`on_update`/`on_unload` callbacks.

Every installed mod receives one activation switch stored in that package's `mod.json`. Missing `enabled` values in third-party EML packages default to disabled; bundled ShroudForge packages explicitly set it to `true`.

Every bundled mod must also declare a useful `ui` page in its manifest. A page should expose only behavior-specific settings or safe actions. Mods must not duplicate the loader activation switch in their own settings.

## Package layout

```text
mod-name/
├── mod.json
└── src/
    └── mod.lua
```

Keep user-facing copy in `mod.json` concise and in English. Use commas or full stops instead of semicolons in prose.

## Validation

The release checks parse every bundled manifest and verify the API-derived execution contract. The release build also rejects unsupported low-level tokens in bundled Lua code.

```powershell
cargo test -p shroudforge-modloader
```
