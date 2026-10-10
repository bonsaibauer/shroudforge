# ShroudForge developer documentation

Welcome! This folder is for people who want to understand or contribute to the ShroudForge project. You do not need these pages to install or play with mods.

For player instructions, start with the [English website guide](https://bonsaibauer.github.io/shroudforge/en/) or the [German website guide](https://bonsaibauer.github.io/shroudforge/de/). They include pictures, a PC and server quickstart, and a step-by-step first-mod lesson.

## Choose a guide

| If you want to… | Read… |
| --- | --- |
| See how the repository and mod loading fit together | [Architecture](Architecture.md) |
| See the concrete refactor plan and target file paths | [Refactor master plan](refactor-masterplan.md) |
| Inspect each file's proposed owner, target and actual review coverage | [Refactor file inventory](refactor-file-inventory.csv) |
| Understand mod files and settings | [Mod packages](mod-packages.md) |
| Follow shared log levels and message rules | [Logging rules](logging.md) |
| Review client, local-host, and dedicated-server behavior | [Multiplayer execution audit](mod-multiplayer.md) |
| Work on the native runtime or its game profiles | [Native runtime](runtime.md) |
| Change how Enshrouded game data is read | [Game data parser](parser.md) |
| Edit notices or link buttons shown in the Modloader | [Modloader content](modloader-content.md) |
| Preview Modloader typography at different window widths | [Modloader typography preview](modloader-typography-preview.html) |

The [root README](../../README.md) is the quick introduction and player starting point. The [mod template](../../templates/mod/README.md) is a practical place to begin creating a mod.

## Keep information easy to find

Each page covers one job and links to the code that owns it. The current schemas and source files define what ShroudForge does. use the guide to find and understand them.
