# ShroudForge Modloader

## Purpose

This component discovers, validates, orders, and runs ShroudForge mod packages. It also contains the Windows bootstrap and the multi-mode `shroudforge.exe` distributed to players.

## Current status

The `1.0.0` package format supports directory and ZIP packages with `mod.json` at the package root and Lua at `src/mod.lua`. Manifests can declare capabilities, dependencies, settings, release notes, and declarative Modloader UI pages.

Native code is reserved for first-party ShroudForge components. User mod packages are Lua packages.

## Activation

Each installed mod stores its activation switch and setting values in `mods/<id>/mod.json`. The Modloader UI edits these fields in place. Missing `enabled` values in third-party packages remain disabled. Bundled ShroudForge mods ship with `enabled: true`.

Disabled packages remain installed and visible in the Modloader UI. They are excluded from startup asset application and runtime execution. Settings and activation are read from the same `mod.json` the UI displays. Enabled asset mods are applied by the bootstrap during early process startup before ShroudForge initializes its live ECS runtime. Direct game starts use this Shroudtopia-style early activation path, but Windows does not pause Enshrouded while the bootstrap worker runs. `shroudforge launch` remains deterministic because it publishes asset changes before creating the game process.

Before applying enabled startup asset mods, the loader restores the clean KFC backup. This removes persistent asset changes from mods that have since been disabled, then reapplies only the currently enabled transformations.

## Layout

| Path | Responsibility |
| --- | --- |
| `src/` | CLI launcher, module-mode dispatch, package discovery, dependency ordering, and runtime loading |
| `package/` | Reusable package registry and manifest validation crate |
| `../config/mods/mod-schema.json` | Manifest schema consumed by package discovery and validation |
| `../templates/mod/` | Minimal Lua mod template |
| `bootstrap/windows/` | Windows `winmm.dll` bootstrap; starts module modes from `shroudforge.exe` |
| `kfc-runtime/` | Native runtime bridge for supported Windows builds |

## Package contract

```text
my-mod/
├── mod.json
└── src/
    └── mod.lua
```

Use the schema as the source of truth for manifest fields. Keep the template synchronized with any required package-format change.

## Development

```powershell
cargo test -p shroudforge-modloader -p shroudforge-package
```

The full Windows bootstrap and release package are built by `build.ps1` from the repository root.
