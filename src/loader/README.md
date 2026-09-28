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
to the calling Lua module. Loose packages and archived packages are supported;
archived packages are extracted with their neighboring files so Windows can
resolve DLL dependencies. Native DLL loading is available in the Windows game
runtime only.

## Windows proxy and native DLL flow

`dbghelp.dll` and `dinput8.dll` are the loader's game-root proxy entrypoints,
alongside the game executable. They forward the corresponding Windows DLL
exports. When the game loads either proxy, it starts the loader. With ShroudForge
installed, the proxy hands startup to the root `winmm.dll` bootstrap, which
starts ShroudForge against the sibling `mods/` folder. The ShroudForge release
ships these proxy DLLs; mod packages do not contain or replace them.

EML mods register package DLLs with `loader.runtime.register_dll(path)`. The API
collects those paths during mod execution; the EML runtime loads them into the
game process at `loader_attach` and unloads them at `loader_detach`. ShroudForge
implements the same EML API through its in-game runtime DLL manager. For loose
and archived mod packages, DLL paths stay within the package and archive files
are extracted with their neighboring files so Windows can resolve dependencies.

XHL's `native-plugin.ini` sidecar is an additional format supported by that
manager. It uses the XHL-defined `XhlNativePluginQuery`,
`XhlNativePluginStart`, and `XhlNativePluginRequestStop` ABI v1 exports; those
names are retained when calling unchanged XHL DLLs.

ShroudForge activation and player settings are stored in the mod package's
`extended.mod.json`; package updates can reset them. Loader configuration is
`shroudforge/config/loader.json`; generated status is
`shroudforge/state/state.json`. EML metadata stays in `mod.json`; ShroudForge
state stays in the adjacent extension. Inline ShroudForge manifest fields and
obsolete extension formats are rejected.

## Source ownership

- `package/src/registry/` owns manifest discovery, parsing, and manifest schemas.
- `api/src/eml/v1/` and `api/src/shroudforge/v1/` own versioned API definitions.
- `workflow/` owns startup, pregame asset preparation, and runtime lifecycle.
- `runtime/` owns native provider integration, approved profiles, and loader diagnostics.
- `modules/` owns UI, debug console, updater, and command entrypoints.

The native provider is an internal CMake target included by the root
`CMakeLists.txt`; it has no separate project or release package. Build the full
Windows release with `build.ps1` from the repository root.
