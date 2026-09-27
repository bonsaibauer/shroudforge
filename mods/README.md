# Bundled Mods

## Purpose

This directory contains the Lua mods shipped with ShroudForge releases. Every child directory is an independent mod package with a `mod.json` manifest and a `src/mod.lua` entrypoint.

## Current mods

| Mod | Execution | Runtime scope | Behavior | Modloader UI |
| --- | --- | --- | --- | --- |
| Flight | Runtime patch | Client | Enables the build-profile-verified native flight operation | Toggle action |
| Infinite Item Split | Runtime ECS | Client | Restores the source stack after the game-selected One, Half, or CustomAmount split | None; the game controls split mode and amount |
| Infinite Item Use | Runtime patch | Client | Applies the build-profile-verified native item-use patch to the matched operation for all items | Lifecycle switch; no per-item exclusions |
| No Fall Damage | Runtime patch | Client | Applies the build-profile-verified native fall-damage patch | Toggle action |
| No Resource Cost | Runtime patch | Client | Applies the build-profile-verified native recipe-cost patch | Lifecycle switch |
| No Stamina Loss | Runtime patch | Client | Prevents depletion through the build-profile-verified native patch | Depletion toggle |
| World Editor | Runtime + export | Client | Copies/pastes bounded voxel regions, captures recipe-resolved placeable props, saves/loads voxel-and-prop blueprints, and exposes profile-backed entity operations | Cursor selection, component discovery/editing, voxel regions, blueprint names, and explicit entity-operation metadata |
| Unlock Blueprints | Startup assets | Client and server | Replaces supported recipe knowledge requirements | Lifecycle information and restart notice |

## Execution and API policy

The loader derives execution from the Lua entrypoint's ShroudForge API use. `runtime.require("game.assets.write")` and asset-mutating methods (`update_asset`, `save_assets`, `reset_assets`, `create_resource`, and `create_content`) select startup asset preparation. Read-only asset calls such as `game.assets.get_resources_by_type` do not request write capability. `runtime.require("runtime.lifecycle")`, lifecycle callbacks, `runtime.ecs`, `runtime.world`, and `runtime.patch` select the in-game runtime. `io.export` and `io.read_export_*` select export support. Runtime ECS, world, and patch operations currently target the Client process; asset and export work can run in Client or Server processes. The loader shows only Client, Server, or Client + Server.

The loader resolves runtime, startup-asset, export, target, and setting apply-phase requirements from the Lua API calls in the mod source. Authors do not add capability or target flags to `mod.json`. Runtime-only mods can be enabled, disabled, and reconfigured live through `on_load`, `on_unload`, and `shroudforge.settings`. Read live settings inside lifecycle callbacks rather than caching them at module initialization. Asset-writing changes take effect on the next game start. A package that uses both runtime and asset APIs is conservatively treated as next-start for activation and settings.

Keep the Lua entrypoint's top level free of gameplay reads and writes. The loader initializes disabled runtime modules so their lifecycle can be activated without restarting, but it denies runtime operations until `on_load` begins. Put gameplay reads and writes inside `on_load`/`on_update`/`on_unload` callbacks. Register UI actions at module initialization and invoke runtime work from their callbacks.

API availability and effect reports show whether a profile operation resolved and whether the runtime observed its immediate effect. They do not prove saved-world persistence or complete parity with native editor transactions. The current World Editor captures only props it can associate with `ItemInfo` placement recipes; entity blueprint paste supports unit-scale props, and its undo is limited to the most recent paste. Treat placement, capture completeness, and persistence as unverified until confirmed in the running game.

Every installed mod receives one activation switch stored in that package's `mod.json`. Missing `enabled` values in third-party EML packages default to disabled; bundled ShroudForge packages explicitly set it to `true`.

Every bundled mod must also declare a useful `ui` page in its manifest. A page should expose only behavior-specific settings or safe actions. Mods must not duplicate the loader activation switch in their own settings. Build-specific patch signatures, bytes, and trampoline data belong in KFC compatibility profiles; Lua contains stable operation names only.

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
