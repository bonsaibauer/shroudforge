# Loader component

The loader is built from one Cargo workspace and one Windows release build. Its
package crate reads EML mod packages, validates ShroudForge extensions, applies
capabilities, and exposes the shared internal model to the loader workflow and
UI.

## Mod package files

```text
mod.json                 required EML manifest
extended.mod.json        optional ShroudForge state, settings, groups, and actions
src/mod.lua              Lua entrypoint
assets/                  optional mod assets
```

The manifest accepts the EML capabilities `patch`, `export`, `runtime`, and
`runtime-register-dll`. The last capability schedules the mod in the live
runtime so its EML `loader.runtime.register_dll(path)` call can load a DLL from
the package. The same API is available to EML Lua mods and reports load errors
to the calling Lua module. Installed mods are unpacked package directories. Download archives are unpacked
by installation before runtime discovery. Neighboring package files remain
available so Windows can resolve DLL dependencies. Native DLL loading is available in the Windows game
runtime only.

## Windows proxy and native DLL flow

`dbghelp.dll` and `dinput8.dll` are the loader's game-root proxy entrypoints,
alongside the game executable. They forward the corresponding Windows DLL
exports. When the game loads either proxy, it starts the loader. With ShroudForge
installed, the proxy hands startup to the root `winmm.dll` bootstrap, which
starts ShroudForge against the sibling `mods/` folder. The ShroudForge release
ships these proxy DLLs. mod packages do not contain or replace them.

EML mods register package DLLs with `loader.runtime.register_dll(path)`. The API
collects those paths during mod execution. the EML runtime loads them into the
game process at `loader_attach` and unloads them at `loader_detach`. ShroudForge
implements the same EML API through its in-game runtime DLL manager. DLL paths stay within the unpacked package and its neighboring files remain
available so Windows can resolve dependencies.
ShroudForge queues each DLL load on an isolated worker so a native DLL initializer
that stalls cannot stop Lua mod activation or frame updates. The runtime log records
when each DLL load is queued, completes, fails, or exceeds the stall threshold.
During game startup, the Debug Console also runs as a hidden log watcher before the
runtime DLL is loaded. It opens on startup/runtime errors and keeps the final log
visible after an unexpected nonzero game exit. Warnings and native DLL stalls do not
open it automatically. Bootstrap phases,
errors, and crash exit codes are recorded in the same `shroudforge/logs/shroudforge-client.log`
used by the Debug Console.

Mods can declare a package-local DLL in `native-plugin.ini`:

```ini
[Plugin]
Enabled=1
Dll=bin/example.dll
```

The DLL path must stay inside the package. ShroudForge loads the DLL through
the Windows loader, which runs its `DllMain`. The only export it looks for is
the optional `ShroudforgeModNativeInit` of the mod native bridge
(`modules/mod-runtime/native_bridge/shroudforge_mod_native.h`), which lets the
DLL register functions for the same mod's Lua code. DLL loading runs on an isolated worker / runtime poll
path so a slow DLL initializer does not block Lua activation. This matches the
behavior of EML's `loader.runtime.register_dll(path)` API.

ShroudForge activation and player settings are stored in the mod package's
`extended.mod.json`. package updates can reset them. Loader configuration is
`shroudforge/config/modloader-config.json`. generated status defaults to
`shroudforge/state.json`. User-selected folders are resolved in
`support/paths.rs`. EML metadata stays in `mod.json`. ShroudForge
state stays in the adjacent extension. Inline ShroudForge manifest fields and
obsolete extension formats are rejected.

The proxy loader is selected with `modloader` in
`shroudforge/config/modloader-config.json`. It defaults to `"shroudforge"`.
Set it to `"eml"` to use EML while keeping the ShroudForge installation available.

## Source ownership

`main.rs` routes into `modes.rs` for worker/window modes and `cli.rs` for CLI
operations. `game_files.rs` selects game filenames. `embedded.rs` retains the
C ABI and `prepare.rs` retains the early asset preparation sequence.

- `packages/` owns manifest discovery, parsing, settings, and manifest schemas.
- `modules/mod-runtime/lua/eml/v1/` and `modules/mod-runtime/lua/shroudforge/v1/` own versioned API definitions.
- `modules/entrypoints/loader/` composes process modes and startup asset preparation.
- `modules/mod-runtime/ingame/` owns the Lua runtime lifecycle and cleanup.
- `modules/game-files/` owns original game files, restoration and file transactions.
- `modules/game-runtime/` owns native provider integration and approved profiles.
- `modules/` contains Modloader, Transport, Debug Console, Updater, diagnostics and command modules. Each module owns its own UI.

The native provider is an internal CMake target included by the root
`CMakeLists.txt`. it has no separate project or release package. Build the full
Windows release with `dev/build/release_build.ps1` from the repository root.
