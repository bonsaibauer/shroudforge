# How ShroudForge is organized

This guide is for people exploring or changing the ShroudForge source code. For an illustrated introduction, see [what ShroudForge does](https://bonsaibauer.github.io/shroudforge/en/#workings) and [how the Lua and KFC runtimes work](https://bonsaibauer.github.io/shroudforge/en/#runtime). For installing mods or getting started as a player, use the [website quickstart](https://bonsaibauer.github.io/shroudforge/en/#play).

## The short version

ShroudForge is a Windows loader for Enshrouded. It finds mod folders, checks their information, prepares changes that need to happen before the game starts, and runs other mod features at the right time. The Modloader, Debug Console, updater, commands, diagnostics, and native runtime are parts of the same product.

### A few useful words

- **Mod package:** the installed folder containing one mod.
- **Manifest:** the mod's information file, **mod.json**.
- **Capability:** a permission a mod must list before it can use certain features.
- **Runtime:** work performed while the game is running.
- **Profile:** reviewed information about one game build, used by supported native operations.
- **ABI:** the agreed set of low-level calls used when two compiled parts talk to each other.

## Where the code lives

~~~text
modules/                       # Product modules and application entrypoints
  entrypoints/                # Loader application and Windows DLL entrypoints
  modloader/                  # Find, read, check and configure mods
    ui/                       # Modloader window and its views
  player-name/                # Resolve the Modloader name from the published native runtime cache
  updater/                    # Check, download and install updates
    ui/                       # Its update views
  loader-config/              # Read and save loader settings and state
  window-management/          # Shared native window sizing and geometry persistence
  hotkeys/                    # SDL scancode bindings, keyboard observation, and shortcut field
  logging/                    # Write logs and display activity
  notifications/              # News, saved events, read state and toasts
  runtime-diagnostics/        # Selectable, time-limited runtime diagnostic jobs
    diagnostics-jobs/         # Categorized *.job.json definitions and scripts
      runtime/                # Runtime collector job definitions
      world-editor/           # Lua job definitions and scripts
      game_inspection/        # Python job definitions, programs and shared imports
    tests/                    # Rust and Python checks with private Lua fixtures
  debug-console/              # Separate log window
  world-editor/               # Separate editor window
  commands/                   # Existing command mode
  transport/                  # Steam messages and network health
  ui-resources/               # Shared display elements and translations
  wiki/                       # Offline help articles, banners and article reader
    articles/                 # One folder per article with locale Markdown and illustration
    ui/                       # Article discovery, Wiki page and contextual help
  mod-runtime/                # Mod sessions, startup actions, callbacks and Lua bindings
    session/                  # Own session state, assets and Lua registrations
    startup/                  # Prepare and commit game-file mod changes
    ingame/                   # Start, update, reload and stop running mods
  game-files/                 # Original game files, restoration and asset transactions
    originals/                # Check, capture and restore immutable original files
    ui/                       # Game-file backup view
  game-runtime/               # Native game access, ABI, Rust bridge and shipped profiles
    bridge/                   # Rust adapters grouped by native operation
    native/                   # C++ owners, ECS, resources, hooks and game-thread queue
    profiles/                 # Complete approved profiles for individual game builds
kfc-parser/                   # Direct parser Git submodule
mods/                         # Independent Lua mod packages, including world-editor
dev/
  build/                      # Product, runtime, website, and complete dev/release build actions
  parser/                     # Prepare the pinned parser submodule integration
  tests/                      # Workspace test action
  profiles/
    cli/                      # Native kfc-runtime-dev command implementation
    capture/                  # Collect executable and live-process evidence
    inspect/                  # Read executable, reflection, and KFC resources
    verify/                   # Check profile and runtime evidence
    checks/                   # Explicit native checks and corpus-test action
    catalogs/                 # Maintained build catalogs and indexes
    schemas/                  # Profile and catalog data contracts
    captures/                 # Ignored local originals and capture results
docs/
  pages/                       # Bilingual Markdown pages published on the website
  website/                     # Website shell, shared code, page integrations, and build tools
  development/                 # Current code ownership and developer workflows
  modding/                     # Mod package and multiplayer guides
  runtime/                     # Runtime API, profiles, and update guides
  examples/                    # Starter mod and reference examples
  evidence/                    # Dated investigations and archived material
build/                         # Generated build and publication output
~~~

The KFC parser is directly mounted at **kfc-parser/**. Cargo consumers use its local crates. Format helpers, schema projection and snapshot export live inside that submodule. Game asset file transactions live under **modules/game-files/**. See [parser integration](integrate-kfc-parser.en.md) for the selected parser revision and integration details.

## What happens when a mod loads?

Installed mods are unpacked folders. A download archive is extracted by the installer. Its **mod.json** tells the loader its name, version, dependencies, and declared permissions. An optional **extended.mod.json** holds ShroudForge settings and extra details. Optional **locales/<locale>.json** files translate mod-authored text for the selected Modloader language.

~~~mermaid
flowchart TD
    A[Find an installed mod folder] --> B[Read mod.json]
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

The manifest access declarations are **patch**, **export**, **runtime**, and **runtime-register-dll**. A mod lists the access it needs in **mod.json**. ShroudForge's in-game runtime APIs use **runtime**. **runtime-register-dll** schedules an EML mod that registers a package-local native DLL. The registration API retains its existing phase, activation and package checks without adding a new capability check. A `native-plugin.ini` declaration or an approved DLL Manager selection uses the common native load plan without requiring a Lua stub or a Lua registration capability. The DLL Manager switch, per-folder activation, process target and exclusions apply to every ShroudForge load route.

Runtime mods have separate Lua environments. The loader selects a matching profile automatically or uses the player's selected profile. When the detected build differs, it reports a warning and enables operations that resolve against the live executable. When a mod stops, the loader asks it to clean up and releases its runtime handles.

Native DLL handles are process-owned. `modules/modloader/dll_manager.rs` detects installed native components and resolves INI declarations, local selections, compatibility profiles and `dinput8-chain.ini` into one ordered plan. Chain entries must resolve inside a direct installed mod package. `mod_profiles.rs` reads data-only mod adapters independently of installed files. Manifest overlays preserve existing capabilities and add profile capabilities without duplicates. `modules/mod-runtime/native_dlls.rs` owns loading, canonical path deduplication, DirectInput provider registration and status. Ordinary DLLs own their internal startup. The bootstrap prepares this plan before releasing the game entrypoint. DLLs stay loaded until process exit, so changed native selections require a game restart. Reading the inventory never executes DLLs or rewrites mod files.

## The two mod information files

- **mod.json** follows the EML format. It contains the mod's identity, version, dependencies, and permissions.
- **extended.mod.json** is an optional ShroudForge file. It contains the enabled state, setting values, groups, actions, setting presets, links, short update notes, and optional `launcher` provenance (`EML` or `SF`). `validate_setting_presets.rs` checks presets when the manifest is read. `ui/apply-setting-presets.ts` applies a chosen preset, or selects its custom value, when a setting changes in the Modloader.

Locale catalogs live at **locales/<locale>.json** inside the same mod folder. `read_mod_locale_catalogs.rs` validates them with `manifest/mod-locale.schema.json`. `resolve_mod_locale.rs` applies them to the Modloader manifest and resolves Lua strings. Lua reads the selected Modloader code through `shroudforge.locale()`.

The UI treats a package without **extended.mod.json** as EML; ShroudForge mods always include the extension. If ShroudForge adds an extension to an EML package, it writes `launcher: "EML"` so the package keeps its original identity. When an extension exists, only that explicit marker is shown as EML; a missing or non-EML marker is shown as SF. The loader checks the extension against its schema before accepting the package.

Player changes are saved into the package's **extended.mod.json**. Updating the package can replace those choices. Setting migration is not supported. See the [mod package guide](../modding/create-mod-package.en.md) for examples and the exact schemas.

An EML package may not have **extended.mod.json**. `ensure_eml_extension.rs` creates that compatibility file when the player first changes the package in ShroudForge. `update_mod_configuration.rs` owns the shared revision check, validation, and write used by settings and enabled-state actions. It reads an existing ShroudForge extension directly and uses the EML compatibility action only when an EML package has no extension yet. Keep this action distinct from downloading or installing a mod package.

## Lua functions and compatibility

Lua declarations and mod execution belong to **modules/mod-runtime/lua/**. EML v1 and ShroudForge v1 keep their versioned contracts. Runtime calls use the standalone **modules/game-runtime/bridge/** crate. The transport module owns its own Lua transport binding. These boundaries keep the same public mod API.

The [API reference](https://bonsaibauer.github.io/shroudforge/en/#api) is generated from the current function definitions. EML v1 keeps its established names and behavior. A breaking change to the ShroudForge API requires affected mods to be updated.

Shortcuts use SDL3 scancodes and mouse button numbers in one setting string,
such as `Ctrl+SDL:29|Mouse:4`. The shortcut editor has separate keyboard and mouse
columns. Each device has at most one chord, with the keyboard first in storage.
`modules/hotkeys/` owns parsing, SDL key names, observation, key capture and
the shared 250 ms debounce gate for configured native UI window toggles. The
Modloader, Debug Console and World Editor use the gate independently. Lua mod
keybind queries continue to use raw press revisions.
Its official SDL3 source submodule is `modules/hotkeys/SDL`, pinned to
`release-3.4.18`. Cargo builds SDL statically through `modules/hotkeys/build.rs`
and compiles `sdl_keyboard.c` against the upstream event types. The existing
release package includes SDL's license and needs no separate SDL DLL.
In-game Lua uses a hidden ShroudForge process for SDL keyboard observation and
receives its states and labels over a pipe. It does not register Raw Input in
Enshrouded's process. Dedicated servers do not start keyboard observation.
`control: "keybind"` settings remain in each mod's `extended.mod.json`, while
native window shortcuts remain in the existing loader configuration. The
Modloader owns declared-binding discovery and conflict validation. Lua mods
use `shroudforge.input.is_keybind_down(settingKey)`,
`shroudforge.input.was_keybind_pressed(settingKey)` and
`shroudforge.input.keybind_label(settingKey)`. The former
`is_key_down("F1")` through `is_key_down("F8")` API was removed in 1.7.6.
Calling it logs a migration error directing mod authors to declare a `keybind`
setting and assign it in Compatibility → Hotkeys.

## Offline help and UI events

`modules/game-runtime/native/main-menu/` owns the native Enshrouded **Mods** entry. It resolves the menu builder and entry renderer from the loaded executable's string references and decoded call graph, independently of ECS profiles. The native callback and `shroudforge.ui.toggle_modloader()` signal the same per-game-process event consumed by `modules/modloader/ui/host/native_menu_toggle.rs`. That host also publishes the existing `general.showNativeModMenu` setting through the visibility event. Native lifecycle and diagnostics include the menu binding.

`modules/wiki/` owns offline help. Articles live at `articles/<article-id>/<locale>.md`, with a shared `illustration.svg` beside the translations. The first H1 supplies the title and the first paragraph supplies the banner summary. `ui/read-wiki-articles.ts` discovers articles and illustrations automatically during the frontend build and selects the available translation. The Wiki page and contextual help use the same reader and article content, without a second route list. Published website articles remain under `docs/pages/`.

`modules/logging/ui/log-ui-event.ts` sends bounded Wiki and Compatibility UI events through the existing host command transport. `modules/modloader/ui/host/record_ui_log.rs` validates the source, level and text and writes each event through `write_activity_at_level`, retaining the configured log filter. Explicit warning and error notifications use the existing command-result toast path. UI events do not repeat native DLL execution results.

## Find a change by its action

A file contains the implementation of the action named by that file. Related helpers stay beside that action. Each module owns its UI. The Modloader window supplies inputs and displays results.

| What you want to change | Open this file |
| --- | --- |
| Find installed mods | `modules/modloader/find_installed_mods.rs` |
| Provide the selected character name to Lua and the Modloader | `modules/game-runtime/native/main-menu/resolve_character_selection.cpp` derives the original selected-entry/name contract from the existing menu builder. `manage_native_main_menu.cpp` observes changes during its existing UI callback and owns copied name bytes. The optional ABI, Rust bridge and `runtime.player.name()` read that cache. The existing runtime status publication carries the same value to `modules/player-name/lib.rs`, which applies the Modloader manual/default-name policy without process access or a reader thread. |
| Read a mod's information and locale catalogs | `modules/modloader/read_mod_manifest.rs`, `modules/modloader/read_mod_locale_catalogs.rs`, `modules/modloader/resolve_mod_locale.rs` |
| Save mod settings | `modules/modloader/save_mod_settings.rs` |
| Parse SDL scancode shortcuts, observe keyboard input, or edit the shared shortcut field | `modules/hotkeys/` |
| Check declared module and mod shortcut conflicts | `modules/modloader/check_hotkey_conflicts.rs` |
| Change a mod's enabled state | `modules/modloader/set_mod_enabled.rs` |
| Create a missing EML compatibility extension | `modules/modloader/ensure_eml_extension.rs` |
| Check the revision, validate, and write changed mod configuration | `modules/modloader/update_mod_configuration.rs` |
| Check conflicting mods | `modules/modloader/check_mod_conflicts.rs` |
| Select the mod load order | `modules/modloader/plan_mod_load_order.rs` |
| Detect native components and resolve the DLL load plan | `modules/modloader/dll_manager.rs` |
| Read compatibility adapters and match automatic or manual selections | `modules/modloader/mod_profiles.rs`, `modules/modloader/mod-profiles/README.md` |
| Load native DLLs once per process | `modules/mod-runtime/native_dlls.rs` |
| Read or save loader settings | `modules/loader-config/read_loader_settings.rs`, `save_loader_settings.rs` |
| Fit, resize, position, save, and reset native Modloader, Updater, and Debug Console window geometry | `modules/window-management/window_geometry.rs`, `reset_window_geometry.rs` |
| Write a log entry | `modules/logging/append_log.rs` |
| Log a Wiki or Compatibility UI event | `modules/logging/ui/log-ui-event.ts`, `modules/modloader/ui/host/record_ui_log.rs` |
| Discover and display offline help articles and contextual banners | `modules/wiki/ui/read-wiki-articles.ts`, `show-wiki.tsx`, `show-wiki-help.tsx`, `show-wiki-article.tsx` |
| Mark news as read | `modules/notifications/mark_news_read.rs` |
| Display a short result message | `modules/notifications/ui/show-toasts.tsx` |
| Run update requests and versioned maintenance jobs | `modules/updater/lib.rs` (`run_request`), `modules/updater/maintenance_jobs.rs`, and the feature implementations under `modules/updater/` |
| Start in-game mod execution | `modules/mod-runtime/ingame/start_ingame_runtime.rs` |
| Run update callbacks | `modules/mod-runtime/ingame/update_mod_callbacks.rs` |
| Read original game-file backup status | `modules/game-files/originals/read_original_backups.rs` |
| Capture original game files | `modules/game-files/originals/capture_original_game_files.rs` |
| Restore verified original game files | `modules/game-files/originals/restore_original_game_files.rs` |
| Publish or recover a changed game-file set | `modules/game-files/game_file_transaction.rs` |
| Apply game-file mods before startup | `modules/entrypoints/loader/asset_preparation/prepare_startup_assets.rs` |
| Select and run a runtime diagnostics job, export its results, or execute Lua, Python, Rust and mod actions | `modules/runtime-diagnostics/diagnostics-jobs/**/*.job.json` defines all built-in jobs using `catalog/diagnostic-job.schema.json` and the shared parser. The module owns `session/`, `catalog/`, `collectors/` and `execution/`. `execution/write_job_exports.rs` owns `JobExports` under the configured exports folder at `diagnostics/<job-id>/`. Messages use the shared Debug Log. Lua jobs live under `world-editor/`, Python jobs and imports under `game_inspection/`, compiled Rust job actions live alongside their definitions under `game_inspection/`, and tests and fixtures remain under `modules/runtime-diagnostics/tests/`. `modules/mod-runtime/ingame/run_diagnostic_job.rs` hosts the active mod sandbox, Rust actions and canonical mod-action call. |
| Measure runtime operations | `modules/runtime-diagnostics/collectors/operation_timings.rs` |
| Report connection health | `modules/transport/report_network_health.rs` |

### Updater migration timing

Updater migrations have two execution points. A `before-update` job runs in the updater that is already installed, before the update queue replaces program files. An `after-install` job needs code from the new updater, so it runs after the system installer has written a successful install receipt. Where the queue worker supports the handoff, it starts the newly installed updater for this phase. The Modloader UI, standalone Updater window, and game bootstrap also ask the installed updater to finish pending after-install jobs, which covers the first release that introduces the handoff. The install receipt and `lastAfterInstall` marker make completed work idempotent. A failed required migration keeps its queue item in error and stops the current updater request so a later start or request can retry it.

Job definitions are embedded in `modules/updater/maintenance_jobs.rs` and copied to the configured updater-jobs folder only when a definition is absent. Existing files are not silently overwritten. When a release changes an existing job's phase or revision, add an explicit upgrade for that persisted definition, as `upgrade_legacy_event_job_phase` does. Add each required migration to `REQUIRED_MIGRATIONS`, register its action in the job parser and executor, include its JSON file in `DEFAULT_JOBS`, and give each distinct migration a new ID or revision. The catalog is not filtered by application version, so keep migration definitions and their actions in later releases that users may install directly until those installations can complete the job. Keep the old data readable until an after-install migration succeeds. A manual ZIP replacement does not write the system install receipt, so it does not trigger after-install jobs.

The module guide is [modules/README.md](../../modules/README.md). Public entry points assemble these actions. A UI command does not carry its own second implementation of mod validation, saving or update installation.

## The native runtime and game profiles

The native C++ runtime is built and shipped with the loader. It is not a separate player download. The Windows bridge checks the runtime interface version before using it.

A **profile** is reviewed information for a particular Enshrouded build and target, such as the client. It tells supported native operations which game structures to use. Approved profiles are under **modules/game-runtime/profiles/** and are included in the runtime during the build.

`modules/game-runtime/native/impact/` owns typed adapters for separately admitted original Impact operations and resolves their callbacks by registered name. `native/ecs/resolve_entity_command_queue.*` validates the original queue and its ownership for those adapters. `native/building/remove_building_prop.cpp` connects entity destruction to the existing ABI and confirms completion through the session-bound entity reader. Queue submission and confirmed world effects are distinct results. The Lua availability policy lives in `modules/mod-runtime/lua/ingame/check_ingame_access.rs`.

`native/code-inventory/resolve_original_call_graph.*` decodes bounded original control flow with Zydis and follows direct calls from registered names. It owns code selection evidence and the shared exact-byte evidence checks used by spawn and voxel writes, not permission to invoke arbitrary functions. `native/impact/submit_building_prop_spawn.*` validates and submits the original building helper with owned command arguments. `native/building/spawn_building_prop.cpp` confirms the exact allocated entity ID and required building components before returning its handle. `native/voxels/write_registered_voxel_grid.*` owns the admitted exact-cell Store writer, aligned chunk preservation, readback, rollback and session fencing. `read_registered_voxel_grid.*` owns the canonical `PinnedRegion` reader used by public reads and writer snapshots/readback. The existing public voxel entrypoint owns bounded retries only for rejection before dispatch.

The Lua World Editor under `mods/world-editor/src/` owns blueprint planning and its session-bound undo journal. `operations.lua` waits for temporary native readiness before placement. `history.lua` restores recorded cells and removes placed props by their handles, independently of the game's bounded undo history. `mod.lua` refreshes readiness and `preview.lua` publishes it through the existing editor-state export.

The dedicated-server coordinators in `operations.lua` and `history.lua` pass the authenticated peer's validated anchor and undo transaction into those same placement and restoration actions. They do not duplicate native feature requirements, grid validation or availability sampling. `p2p.lua` owns framing, transfer integrity, authorization callbacks and result delivery. The editor callback interval is 100 ms. Additional client readiness observations run at most once per two accumulated callback seconds and pause during active work.

The online capture action sends the selection through `p2p.lua`, runs the canonical capture on the server and returns V7 content for client validation and `library.lua::save_capture`. Local and remote captures share unique naming and file writes. `p2p.lua` owns the bounded FIFO request queue, separate peer upload buffers and peer/request receipts. `operations.lua` keeps each peer's latest native undo journal and only activates one journal during a serialized operation. `history.lua` restores and releases only the matching peer's journal. A confirmed journal does not block another paste action. Partial recovery does. These journals remain session-bound and in memory.

See [Lua and native runtime](../runtime/native-runtime-and-game-profiles.en.md) for the runtime, interface, and profile workflow.

## Files in a player installation

The release ZIP contains these main program files:

| Installed file | What it is for |
| --- | --- |
| **winmm.dll** | Starts the ShroudForge bridge beside Enshrouded. |
| **dbghelp.dll** | Windows proxy and normal game-imported entry to the ShroudForge bridge. |
| **dinput8.dll** | Alternative bootstrap and DirectInput chain host for native mods selected by the DLL Manager. |
| **shroudforge/shroudforge-runtime.dll** | Loader library used by the bridge. |
| **shroudforge/shroudforge.exe** | Main program: launcher, command-line tools, and linked ShroudForge modules. |
| **shroudforge/shroudforge-updater.exe** | Owns update checks and actions, queued work, versioned migration jobs, and recycle-bin operations. The Modloader UI sends actions to its request mode. |
| **shroudforge/kfc-runtime.dll** | Native runtime used for approved in-game operations. |
| **shroudforge/version.json** | Release version and list of files managed by system updates. |
| **shroudforge/config/modloader-config.json** | Loader and module settings, including custom storage locations. |
| **mods/<id>/** | Mod packages included in the release. |

ShroudForge also creates or updates these working files as you use it:

| Installed path | What it is for |
| --- | --- |
| **shroudforge/state.json** | Saved loader, mod, update, and diagnostic status. |
| **shroudforge/logs/shroudforge-client.log** / **shroudforge/logs/shroudforge-server.log** | Current client or dedicated-server ShroudForge log; each process writes to its role-specific file and rotated logs use timestamped names in this folder. |
| **shroudforge/ui/** | Mod downloads, queued actions, and removed-mod storage. |
| **shroudforge/updates/** | Update downloads, queued work, and backups. |
| **shroudforge/runtime/** | Runtime status and requests shared with the running game. |
| **shroudforge/runtime/profiles/mod-profiles/** | Update-managed JSON compatibility adapters for mods. These are separate from game-build profiles. |
| **shroudforge/runtime/diagnostics/jobs/** | User-defined diagnostic job manifests. This folder follows a custom Runtime storage path when configured. |
| **shroudforge/cache/** | Saved parser and Lua type information, plus temporary bundled diagnostic sources under **diagnostics/**. |
| **shroudforge/exports/** | Mod exports and diagnostic result files under **diagnostics/&lt;job-id&gt;/**. |

The Settings → Storage locations page can redirect the mods, status, logs, cache, exports, updates, UI-data, and runtime-data folders. Those paths resolve through `modules/loader-config/resolve_installation_paths.rs`; modules should use that resolver instead of constructing paths themselves. The game install root, configuration file, and injected DLL locations stay fixed because the Windows bootstrap needs them before the configurable storage paths can be read. Selecting a folder changes where new files are written; it does not copy existing files. A custom mods folder outside the game directory may not be discovered by EML.

System updates manage the program files and bundled mods. They do not replace **modloader-config.json**, the selected status file, the write lock, or the mod-update check lock. Bundled mod files are installed under the configured mods folder.

## Build and development actions

Run commands from the repository root. Each PowerShell file performs the action named by its filename and contains its actual build or package steps.

Initialize the Git submodules before building. The pinned parser revision includes the recorded integration, checked with `./dev/parser/prepare_parser.ps1`. The script can also prepare the legacy revision from its recorded patch. Both GitHub workflows run the same full build, tests, and packaging through `dev/build/dev_build.ps1` or `dev/build/release_build.ps1`. A version-branch push publishes a prerelease. Manually running the release workflow publishes a regular release and marks it latest. Both scripts run this preparation step themselves.

| Action | File | What it does |
| --- | --- | --- |
| Build the Modloader and its UI | `./dev/build/build_modloader.ps1` | Builds the Rust workspace and copies the Modloader binaries. |
| Build only the Modloader UI | `./dev/build/build_modloader_frontend.ps1` | Builds the Modloader frontend. |
| Build the native runtime | `./dev/build/build_kfc_runtime.ps1` | Configures and builds `kfc-runtime.dll`. |
| Build the Windows bootstrap | `./dev/build/build_windows_bootstrap.ps1` | Builds the Windows bootstrap project. |
| Build Windows proxies | `./dev/build/build_windows_proxies.ps1` | Builds the EML-compatible proxy DLLs. |
| Scan packaged release files with Microsoft Defender | `./dev/build/scan_release_with_defender.ps1` | Updates Defender signatures, scans release package contents, and writes machine-readable scan results. |
| Scan release ZIP files with the VirusTotal API | `./dev/build/scan_release_with_virustotal.sh` | Uploads each run's release ZIP, waits for the API analysis, and writes the full final Analysis response together with its machine-readable summary in one JSON report. |
| Write per-scanner and unified release security reports | `./dev/build/write_release_security_report.py` | Normalizes scanner output and writes scanner findings, job outcomes, and unified Markdown and JSON reports. |
| Package release security reports | `./dev/build/package_release_security_report.py` | Bundles the displayed report and all available scanner files into one release ZIP and writes its SHA-256 sidecar. |
| Locate Visual Studio | `./dev/build/find_visual_studio.ps1` | Finds the installed x64 C++ toolchain for native builds. |
| Prepare the parser submodule | `./dev/parser/prepare_parser.ps1` | Accepts the pinned parser integration or applies the recorded patch to the legacy revision. |
| Package existing build outputs | `./dev/build/package_release.ps1` | Stages product files, writes the version manifest, and creates the ZIP and checksum. |
| Run workspace tests | `./dev/tests/run_workspace_tests.ps1` | Runs the bootstrap checks and Rust workspace tests. |
| Build for the automatic dev prerelease | `./dev/build/dev_build.ps1` | Runs the same full build, tests, and packaging as the release entry point. |
| Build for a manual release | `./dev/build/release_build.ps1` | Runs the same full build, tests, and packaging as the dev entry point. |
| Build and validate the website | `./dev/build/check_website.ps1` | Runs the website build and validation command. |

`dev/build/find_visual_studio.ps1` is a shared helper called by the native runtime and Windows bootstrap build actions. It locates the installed C++ toolchain and is not a separate product build.

Both build entry points run the same full build sequence and call the concrete build actions. Tests are an explicit action, `dev/tests/run_workspace_tests.ps1`. Nothing in these instructions creates a second game installation.

For executable inspection, captures and profile preparation, start with [dev/profiles/README.md](../../dev/profiles/README.md). Sources, capture inputs and generated outputs have separate owners. Approved runtime profiles remain in **modules/game-runtime/profiles/**. Local captures remain in **dev/profiles/captures/**. Generated release and website files belong in **build/**.

The **shroudforge/** folder in a player installation is generated product data. It is not a source-code component of this repository. Its installed storage locations are described above and remain configurable through the loader settings.
