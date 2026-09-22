# ShroudForge Modules

## Purpose

Modules are first-party companion processes shipped with ShroudForge. They are trusted platform components, not user mods.

## Current modules

| Directory | Executable | Responsibility | Status |
| --- | --- | --- | --- |
| `commands/` | `shroudforge-commands.exe` | Central command endpoint | Included in releases |
| `debug-console/` | `shroudforge-debug-console.exe` | Searchable view of Enshrouded and ShroudForge logs | Included in releases |
| `modloader-ui/` | `shroudforge-modloader-ui.exe` | Mod management, settings, news, compatibility, and updates | Included in releases |
| `updater/` | `shroudforge-updater.exe` | Applies staged updates after Enshrouded exits and rolls back failed installs | Included in releases |

Each packaged module except the updater has a `module.json` beside its executable. The updater is launched from the dedicated `Shroudforge_Updater` release directory.

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
