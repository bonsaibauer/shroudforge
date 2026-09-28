# ShroudForge API

## Purpose

This crate provides the Lua API used by ShroudForge mods. It combines parsed game data, the verified compatibility contract, settings, logging, asset access, and runtime ECS operations in one sandboxed mod environment. The loader derives execution scope and target from API use in `src/mod.lua`; authors do not maintain capability, target, or apply-phase flags in `mod.json`.

## Current status

The API is part of the `1.0.0` workspace and supports two execution phases.

- Asset API users can inspect, export, and modify supported game resources during startup preparation.
- Runtime API users can query and update supported ECS components while Enshrouded is running. Runtime-only packages support live lifecycle and settings changes.
- Packages that use startup asset APIs require the next game start for those changes to take effect.
- Runtime mods should call `shroudforge.settings.get` from lifecycle callbacks instead of caching a value during module initialization when that setting must change live.
- Capability checks restrict sensitive operations such as asset writes and exports.
- Lua definition files under `definitions/` provide editor and documentation metadata.

The available game types depend on the compatibility profile selected for the installed Enshrouded build.

## Main areas

| Path | Responsibility |
| --- | --- |
| `src/env/` | Lua globals such as `game`, `runtime`, and `shroudforge` |
| `src/runner/` | Mod lifecycle and Lua state management |
| `src/cache/` | File state tracking for generated or exported data |
| `src/definition/` | Lua definition generation |
| `src/eml/v1/definitions/` | EML v1 Lua declarations |
| `src/shroudforge/v1/definitions/` | ShroudForge v1 Lua declarations |

## Read a setting in Lua

The key inside `extended.mod.json` is the key your mod reads. For example, the starter file defines `greeting` under `settings`; its Lua code calls:

```lua
local greeting = shroudforge.settings.get(
    "greeting",
    "Hello from the ShroudForge runtime!"
)
```

The first argument is the setting key. The second is a fallback for a missing value. Keep the fallback the same kind of value as the setting's `value`. Read settings in a runtime callback or action when the latest player choice should take effect. The loader updates the active runtime settings after a player saves a change; code that cached the value once during module startup still holds its old local value.

The Modloader saves a changed player value in `extended.mod.json`, under `settings.<key>.value`. The setting key must also be listed in `groups[].settings` for its control to appear in the Modloader. The [website guide](https://bonsaibauer.github.io/shroudforge/en/#manifests) shows both files, the Lua lookup, and a preview.
## Development

Run the crate checks from the repository root.

```powershell
cargo test -p shroudforge-api
```

Public API changes update the Rust implementation and Lua declarations together. EML v1 remains the current EML contract. ShroudForge ships only its current API contract; breaking changes require updating the affected mods. The website index is generated from the active declarations.
