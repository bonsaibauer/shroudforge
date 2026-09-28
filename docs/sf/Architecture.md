# How ShroudForge is organized

This guide is for people exploring or changing the ShroudForge source code. For installing mods or getting started as a player, use the [illustrated website guide](https://bonsaibauer.github.io/shroudforge/en/).

## The short version

ShroudForge is a Windows loader for Enshrouded. It finds mod folders, checks their information, prepares changes that need to happen before the game starts, and runs other mod features at the right time. The Modloader, Debug Console, updater, commands, diagnostics, and native runtime are parts of the same product.

### A few useful words

- **Mod package:** the folder or ZIP containing one mod.
- **Manifest:** the mod's information file, **mod.json**.
- **Capability:** a permission a mod must list before it can use certain features.
- **Runtime:** work performed while the game is running.
- **Profile:** reviewed information about one game build, used by supported native operations.
- **ABI:** the agreed set of low-level calls used when two compiled parts talk to each other.

## Where the code lives

~~~text
shroudforge/
├── src/
│   ├── bootstrap/windows/       # Windows entry point and native bridge
│   ├── loader/
│   │   ├── api/                 # EML and ShroudForge Lua functions
│   │   ├── package/             # Mod files, schemas, settings, and state
│   │   ├── compatibility/       # Game and runtime support checks
│   │   ├── modules/             # Five built-in modules and shared UI styles
│   │   ├── runtime/             # Native runtime, profiles, diagnostics
│   │   └── workflow/            # Mod loading and startup steps
│   └── parser/                  # Enshrouded data reader
├── mods/<id>/                   # Mods included with ShroudForge
├── templates/mod/               # Starting point for a new mod
├── docs/                        # Player and developer guides
├── website/                     # Bilingual guide and searchable reference
├── Cargo.toml                   # Rust workspace
├── CMakeLists.txt               # Native runtime build
└── build.ps1                    # Windows release build
~~~

The KFC parser is an upstream project kept at a fixed revision under **src/parser/kfc-parser/**. ShroudForge's own parser adapter lives next to it. This keeps ShroudForge changes separate from the upstream source.

## What happens when a mod loads?

A mod can be a folder or a ZIP. Its **mod.json** tells the loader its name, version, dependencies, and declared permissions. An optional **extended.mod.json** holds ShroudForge settings and extra details.

~~~mermaid
flowchart TD
    A[Find a mod folder or ZIP] --> B[Read mod.json]
    B --> C[Read optional extended.mod.json]
    C --> D[Check files, permissions, target, and dependencies]
    D --> E[Choose when each part can run]
    E --> F{What does the mod do?}
    F -->|Prepare game files| G[Make changes before the game starts]
    F -->|Use exported files| H[Use the allowed export folder]
    F -->|Work while playing| I[Check runtime and game support]
    I --> J[Start src/mod.lua in a separate Lua environment]
    J --> K[Run load, update, and unload callbacks]
    G --> L[Show the result and any problem]
    H --> L
    K --> L
~~~

The manifest access declarations are **patch**, **export**, **runtime**, and **runtime-register-dll**. A mod lists the access it needs in **mod.json**. ShroudForge's in-game runtime APIs use **runtime**. **runtime-register-dll** schedules an EML mod that registers a package-local native DLL for the runtime. The registration API checks the current phase and package path, but does not check this specific capability.

Runtime mods have separate Lua environments. The loader checks the game build and approved profile before providing supported operations. When a mod stops, the loader asks it to clean up and releases its runtime handles.

## The two mod information files

- **mod.json** follows the EML format. It contains the mod's identity, version, dependencies, and permissions.
- **extended.mod.json** is an optional ShroudForge file. It contains the enabled state, setting values, groups, actions, links, and short update notes.

A package with only **mod.json** can be read as an EML mod. Adding **extended.mod.json** makes it a ShroudForge mod. The loader checks the extension against its schema before accepting the package.

Player changes are saved into the package's **extended.mod.json**. Updating the package can replace those choices. Setting migration is not supported. See [Mod packages](mod-packages.md) for examples and the exact schemas.

## Lua functions and compatibility

There is one source folder for the Lua API: **src/loader/api/src/**. EML v1 and ShroudForge v1 functions each have their own versioned folder. Shared host code connects mods to the loader; it does not define a second public API.

The [API reference](https://bonsaibauer.github.io/shroudforge/en/#api) is generated from the current function definitions. EML v1 keeps its established names and behavior. A breaking change to the ShroudForge API requires affected mods to be updated.

## The five built-in modules

Each folder with a Cargo.toml under **src/loader/modules/** is a built-in ShroudForge module. The root loader Cargo.toml includes five of them:

| Module | What it does |
| --- | --- |
| **commands/** | Provides the command-module entry point and reports its availability. |
| **debug-console/** | Shows and filters messages from Enshrouded and ShroudForge. |
| **modloader-ui/** | Manages mods, settings, notices, compatibility details, and updates. |
| **runtime-diagnostics/** | Collects optional, time-limited runtime and mod activity details. |
| **updater/** | Queues mod install and update work, and applies staged ShroudForge updates. |

The **ui-shared/** folder contains shared visual styles used by the interfaces. It has no Cargo.toml and does not run as its own module.

The module sources, Cargo files, and descriptors are in [src/loader/modules/](../../src/loader/modules/README.md).

## The native runtime and game profiles

The native C++ runtime is built and shipped with the loader. It is not a separate player download. The Windows bridge checks the runtime interface version before using it.

A **profile** is reviewed information for a particular Enshrouded build and target, such as the client. It tells supported native operations which game structures to use. Approved profiles are under **src/loader/runtime/profiles/** and are included in the runtime during the build.

See [Native runtime](runtime.md) for the runtime, interface, and profile workflow.

## Files in a player installation

The release ZIP contains these main program files:

| Installed file | What it is for |
| --- | --- |
| **winmm.dll** | Starts the ShroudForge bridge beside Enshrouded. |
| **shroudforge/shroudforge-runtime.dll** | Loader library used by the bridge. |
| **shroudforge/shroudforge.exe** | Main program: launcher, command-line tools, and linked ShroudForge modules. |
| **shroudforge/shroudforge-updater.exe** | Helper that installs a queued ShroudForge update after the game closes. |
| **shroudforge/kfc-runtime.dll** | Native runtime used for approved in-game operations. |
| **shroudforge/version.json** | Release version and list of files managed by system updates. |
| **shroudforge/config/loader.json** | Loader and module settings. |
| **mods/<id>/** | Mod packages included in the release. |

ShroudForge also creates or updates these working files as you use it:

| Installed path | What it is for |
| --- | --- |
| **shroudforge/state/state.json** | Saved loader, mod, update, and diagnostic status. |
| **shroudforge/shroudforge.log** | Latest ShroudForge messages. |
| **shroudforge/logs/** | Earlier log files. |
| **shroudforge/ui/** | Mod downloads, queued actions, and removed-mod storage. |
| **shroudforge/updates/** | Update downloads, queued work, and backups. |
| **shroudforge/runtime/** | Runtime status and requests shared with the running game. |
| **shroudforge/cache/** | Saved parser and Lua type information used to speed up work. |
| **shroudforge/exports/** | Files created by mods that use the export feature. |

System updates keep **loader.json**, **state.json**, and the write lock. Extra mod folders and logs are outside the release's managed file list and stay in place. Mods included in a release are updated with it, so settings stored inside one of those mod packages may change with that package.

## Main parts and their owners

| Area | What belongs there |
| --- | --- |
| **src/loader/package/** | Mod reader, schemas, configuration defaults, and generated status. |
| **src/loader/api/src/eml/v1/** and **shroudforge/v1/** | Public, versioned Lua functions. |
| **src/loader/api/src/env/** and **runner/** | Shared Lua host and separate mod environments. |
| **src/loader/workflow/** | Finding, preparing, starting, and stopping mods. |
| **src/loader/compatibility/** | Checks for game version, target, and runtime support. |
| **src/loader/modules/** | Features linked into the main program. |
| **src/loader/runtime/** | Native runtime, profiles, diagnostics, and maintainer tools. |
| **src/parser/** | ShroudForge's adapter for reading game data. |
| **mods/** | Mods included with the release. |
| **website/** | Player guide and generated reference. |

Each feature keeps its code and rules close together. This makes it easier to find the right place when behavior changes.

## For contributors

The Windows release is built from the root workspace. The root CMake build includes the native runtime; **build.ps1** assembles the release.

When changing a rule, check its source and schema together. Good starting points are **src/loader/package/src/registry/** for mod files, **src/loader/api/src/** for Lua functions, and **src/loader/runtime/profiles/** for supported game builds.
