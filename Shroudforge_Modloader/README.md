# ShroudForge Modloader

## Purpose

This component discovers, validates, orders, and runs ShroudForge mod packages. It also contains the Windows bootstrap and the command-line launcher distributed as `shroudforge.exe`.

## Current status

The `1.0.0` package format supports directory and ZIP packages with `mod.json` at the package root and Lua at `src/mod.lua`. Manifests can declare capabilities, dependencies, settings, release notes, and declarative Modloader UI pages.

Native code is reserved for first-party ShroudForge components. User mod packages are Lua packages.

## Activation

Every installed mod has one loader-managed activation switch. Its state is stored at `mods.<id>.enabled` in `config/shroudforge.json` and defaults to `true`.

Disabled packages remain installed and visible in the Modloader UI. They are excluded from both pregame and runtime execution after the next game restart. Behavior-specific settings remain in the package manifest and must not duplicate this activation switch.

Before applying enabled pregame asset mods, the loader restores the clean KFC backup. This removes persistent asset changes from mods that have since been disabled, then reapplies only the currently enabled transformations.

## Layout

| Path | Responsibility |
| --- | --- |
| `src/` | Launcher, package discovery, dependency ordering, and runtime loading |
| `package/` | Reusable package registry and manifest validation crate |
| `schemas/mod.schema.json` | Public JSON Schema for `mod.json` |
| `templates/lua-mod/` | Minimal Lua mod template |
| `bootstrap/windows/` | Windows `winmm.dll` bootstrap |
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
