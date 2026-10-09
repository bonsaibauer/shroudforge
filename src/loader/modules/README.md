# Built-in ShroudForge modules

The folders here contain parts of ShroudForge itself. They are built with the loader and are not mods that players need to install.

There are **six Rust module crates**. The **ui-shared** folder contains shared styles for the user interfaces; it is not a seventh running module.

Other loader parts, such as the API, package reader, compatibility checks, parser, and startup workflow, live in neighboring folders under **src/loader/**.

## The six Rust module crates

| Folder | What it does |
| --- | --- |
| **commands/** | Provides a command-module entry point. It can report whether it is available and writes an availability message to the ShroudForge log. |
| **debug-console/** | Opens a window for viewing and filtering Enshrouded and ShroudForge log messages. The in-game shortcut is **F10**. |
| **modloader-ui/** | Shows installed and discoverable mods, settings, notices, compatibility details, and update controls. The in-game shortcut is **F9**. |
| **runtime-diagnostics/** | Collects optional, time-limited runtime health and mod activity details. Its controls are in the Modloader settings. |
| **updater/** | Queues mod installation and update work, and applies staged ShroudForge updates. |
| **world-editor-ui/** | Currently contains the World Editor window, blueprint-state reading and screenshot/cover handling for the separate `mods/world-editor` package. The refactor plan replaces this special coupling with a generic mod UI host and moves editor-specific behavior into the mod package. |

Each Rust module has its own Cargo.toml. The root loader lists these crates as dependencies in **src/loader/Cargo.toml**. Most are libraries linked into **shroudforge.exe** and started by a command-line mode; the updater also builds **shroudforge-updater.exe**. These crates are not all separate processes or dynamically loaded plugins. The [refactor master plan](../../../docs/sf/refactor-masterplan.md) maps their current entry points to proposed task-specific files.

## Shared interface styles

**ui-shared/tokens.css** contains shared colors and other visual values used by the user interfaces. It is a style file; it does not have a Cargo crate or a separate process.

## Modloader interface

The Modloader window's interface is built with React and TypeScript under **modloader-ui/ui/**. Its built files are included in the loader.

To build the interface while working on it:

~~~powershell
Set-Location src/loader/modules/modloader-ui/ui
npm ci
npm run build
~~~

English is the source language. Translations are under **ui/src/locales/**.

## Run module checks

Run checks from the repository root:

~~~powershell
cargo test -p shroudforge-commands -p shroudforge-debug-console -p shroudforge-modloader-ui -p shroudforge-runtime-diagnostics -p shroudforge-updater -p shroudforge-world-editor-ui
~~~

These commands are for contributors changing ShroudForge. Players do not need them.
