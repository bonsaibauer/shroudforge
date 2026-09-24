# ShroudForge Modules

## Purpose

Modules are first-party companion processes built into `shroudforge.exe`. They are trusted platform components, not user mods. Their UI and helper modes still run in separate processes when needed.

## Current modules

| Directory | Executable | Responsibility | Status |
| --- | --- | --- | --- |
| `commands/` | `shroudforge.exe --commands` | Central command endpoint | Linked into launcher |
| `debug-console/` | `shroudforge.exe --debug-console` | Searchable view of Enshrouded and ShroudForge logs | Linked into launcher |
| `modloader-ui/` | `shroudforge.exe --module-ui` | Mod management, settings, news, compatibility, and updates | Linked into launcher |
| `updater/` | `shroudforge.exe --update-worker` | Applies staged updates after Enshrouded exits and rolls back failed installs | Linked into launcher |
| `runtime-diagnostics/` | `shroudforge.exe --runtime-diagnostics` | Explicit, bounded native inspection with shared logging | Linked into launcher; native probes embedded |

Module sources and their descriptors remain in this repository. The player ZIP contains no `Shroudforge_Modules` or `Shroudforge_Updater` folder.

## Modloader UI frontend

The Modloader UI uses a React and TypeScript frontend under `modloader-ui/ui`. Its production output is embedded into the Rust executable.

```powershell
Set-Location Shroudforge_Modules/modloader-ui/ui
npm ci
npm run build
```

English is the source locale. Other locale files must contain the same keys and can be checked with the frontend scripts.

## Development

Run module checks from the repository root.

```powershell
cargo test -p shroudforge-commands -p shroudforge-debug-console -p shroudforge-modloader-ui -p shroudforge-updater
```
