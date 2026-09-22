# ShroudForge API

## Purpose

This crate provides the Lua API used by ShroudForge mods. It combines parsed game data, the verified compatibility contract, package capabilities, settings, logging, asset access, and runtime ECS operations in one sandboxed mod environment.

## Current status

The API is part of the `1.0.0` workspace and supports two execution phases.

- Pregame mods can inspect, export, and modify supported game resources.
- Runtime mods can query and update supported ECS components while Enshrouded is running.
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
| `definitions/` | Public Lua API declarations |

## Development

Run the crate checks from the repository root.

```powershell
cargo test -p shroudforge-api
```

Public API changes should update the Rust implementation, the files in `definitions/`, and any affected examples or website data together.
