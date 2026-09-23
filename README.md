# ShroudForge

[![Made With Love](https://img.shields.io/badge/Made%20with%20%E2%9D%A4%EF%B8%8F-by%20bonsaibauer-green)](https://github.com/bonsaibauer)
[![Repository](https://img.shields.io/badge/Repository-shroudforge-blue?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge)
[![License](https://img.shields.io/badge/License-MIT-blue)](https://github.com/bonsaibauer/shroudforge/blob/main/LICENSE)
[![Visitors](https://visitor-badge.laobi.icu/badge?page_id=bonsaibauer.shroudforge)](https://github.com/bonsaibauer/shroudforge)
[![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?style=flat&logo=windows&logoColor=white)](https://github.com/bonsaibauer/shroudforge)

[![Latest Release](https://img.shields.io/github/v/release/bonsaibauer/shroudforge?label=Latest%20Release)](https://github.com/bonsaibauer/shroudforge/releases/latest)
[![Report Bug](https://img.shields.io/badge/Report-Bug-critical?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge/issues/new?template=bug_report.yml)
[![Request Feature](https://img.shields.io/badge/Request-Feature-green?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge/issues/new?template=feature_request.yml)
[![Compatibility Problem](https://img.shields.io/badge/Report-Compatibility_Problem-important?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge/issues/new?template=version_mismatch.yml)
![GitHub Stars](https://img.shields.io/github/stars/bonsaibauer/shroudforge?style=social)
![GitHub Forks](https://img.shields.io/github/forks/bonsaibauer/shroudforge?style=social)

ShroudForge is a modding platform for **Enshrouded**. You can use ready-made mods
or create your own mods with Lua. Every mod uses the same simple format: a
`mod.json` file and a `src/mod.lua` file.

![ShroudForge Modloader UI](images/modloader-ui.png)

## Quickstart

1. Close Enshrouded.
2. Open the [latest ShroudForge release](https://github.com/bonsaibauer/shroudforge/releases/latest).
3. Download `shroudforge-<version>-<build>.zip`.
4. Extract the contents of the `game` directory into your Enshrouded installation directory, next to `enshrouded.exe`.
5. Start the game with:

   ```powershell
   .\shroudforge.exe launch "."
   ```

6. Press `F9` in the game to open the modloader.
7. Press `F10` to open the Debug Console.

The typical Steam path is:

```text
C:\Program Files (x86)\Steam\steamapps\common\Enshrouded
```

Updates are shown in the modloader. A staged update is installed after the game
exits. Your own mods, settings, and logs are preserved.

## Bundled mods

| Mod | What does it do? | Target |
| --- | --- | --- |
| **Flight** | Lets you fly continuously. It can also prevent fall damage while flying. | Client |
| **Infinite Item Split** | Preserves the quantity in the original stack when splitting it. | Client |
| **Infinite Item Use** | Restores used items so they are not permanently consumed. | Client |
| **No Fall Damage** | Prevents your character from taking fall damage. | Client |
| **No Resource Cost** | Prevents recipes from consuming their listed resources. | Client and server |
| **No Stamina Loss** | Prevents your character's stamina from decreasing. | Client |
| **Unlock Blueprints** | Unlocks the supported crafting recipes. | Client and server |

Mods are stored in the `mods` directory. Each mod can be installed as a directory
or a ZIP file. In both cases, `mod.json` and `src/mod.lua` must be located directly
at the package root.

## Bundled modules

Modules are first-party ShroudForge components, not community mods.

| Module | Purpose |
| --- | --- |
| **Modloader UI** | Displays mods, settings, messages, and updates. Open it with `F9`. |
| **Debug Console** | Displays `enshrouded.log` and `shroudforge.log` with search and filters. Open it with `F10`. |
| **Commands** | Provides the central command interface for ShroudForge. |
| **Updater** | Checks for and installs staged ShroudForge updates after the game exits. |

## API documentation

Want to know which functions, game types, fields, and resources you can use in a
mod?

**[Open the ShroudForge API documentation](https://bonsaibauer.github.io/shroudforge/)**

| Namespace | Purpose |
| --- | --- |
| `game.*` | Read or modify game types, resources, and game data. |
| `runtime.*` | Work with the running game world and its components. |
| `shroudforge.*` | Use logging, settings, mod UI, and notifications. |

The API site contains the current catalog for Enshrouded build `1076226`, with
14,398 types, 42,729 fields, and 131 resource types.

## Build your first mod

### 1. Create the directory structure

```text
my-first-mod/
├── mod.json
└── src/
    └── mod.lua
```

You can also copy the ready-made
[Lua template](https://github.com/bonsaibauer/shroudforge/tree/main/Shroudforge_Modloader/templates/lua-mod).

### 2. Create `mod.json`

```json
{
  "id": "yourname.my-first-mod",
  "name": "My First Mod",
  "version": "1.0.0",
  "api": "^1.0.0",
  "capabilities": ["runtime"],
  "dependencies": [],
  "target": "client",
  "description": "My first ShroudForge mod."
}
```

| Field | Description |
| --- | --- |
| `id` | Unique identifier. Lowercase letters, numbers, periods, and hyphens are allowed. |
| `name` | Name shown to players in the modloader. |
| `version` | Your mod's version in `MAJOR.MINOR.PATCH` format. |
| `api` | Required ShroudForge API version. |
| `capabilities` | Features required by the mod. |
| `dependencies` | Other mods that must be installed first. |
| `target` | `client`, `server`, or `both`. |
| `description` | A short, simple description. |

| Capability | Purpose |
| --- | --- |
| `runtime` | Work with the running game world. |
| `assets-write` | Modify game resources before the game starts. |
| `export` | Export data from game resources. |

### 3. Write Lua code

Save your code in `src/mod.lua`:

```lua
shroudforge.log.info("My First Mod was loaded")

local enabled = shroudforge.settings.get("enabled")

if enabled then
    shroudforge.log.info("The mod is enabled")
end
```

A mod with `runtime` runs while the game is running. A mod with `assets-write`
changes resources before the game starts. Use the
[API search](https://bonsaibauer.github.io/shroudforge/) to find suitable types
and functions.

### 4. Add a setting

For example, add a toggle to `mod.json`:

```json
"settings": [
  {
    "key": "enabled",
    "type": "boolean",
    "control": "toggle",
    "label": "Enable mod",
    "description": "Enables or disables the mod.",
    "default": true
  }
]
```

The modloader supports toggles, checkboxes, text fields, number fields, sliders,
select controls, key bindings, and colors. A mod can also display custom sections,
tabs, notices, and safe button actions. The complete structure is documented in
[`mod.schema.json`](https://github.com/bonsaibauer/shroudforge/blob/main/Shroudforge_Modloader/schemas/mod.schema.json).

### 5. Test the mod

1. Copy the mod directory to `Enshrouded\mods`.
2. Start Enshrouded with `shroudforge.exe launch "."`.
3. Open the modloader with `F9`.
4. Check the logs with `F10` if something does not work.
5. Make sure your mod cleanly resets its state when it shuts down.

### 6. Distribute the mod as a ZIP file

The ZIP file must have this structure at its root:

```text
mod.json
src/mod.lua
assets/            optional
```

Do not add an extra top-level directory to the ZIP file. Native DLLs, machine
code, and custom executable files do not belong in a ShroudForge mod.

## Project structure for developers

| Directory | Contents |
| --- | --- |
| `Shroudforge_Parser` | Reads the current Enshrouded data. |
| `Shroudforge_Compatibility` | Checks the data against the supported game build. |
| `Shroudforge_API` | Provides the Lua API. |
| `Shroudforge_Modloader` | Loads, validates, and starts mods. |
| `Shroudforge_Modules` | Contains the Modloader UI, Debug Console, Commands, and Updater. |
| `mods` | Contains the bundled Lua mods. |
| `site` | Contains the public API site. |

### Check the project

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
npm run site:check
```

The Modloader UI is built in `Shroudforge_Modules/modloader-ui/ui`. Translations
are stored as JSON files under `ui/src/locales`; English is the source language.
The complete Windows release is built with `build.ps1`.

## Getting help and reporting problems

Open a [new GitHub issue](https://github.com/bonsaibauer/shroudforge/issues/new)
and include a short description of the problem and the relevant section of
`shroudforge.log`.

## License

ShroudForge is available under the
[MIT License](https://github.com/bonsaibauer/shroudforge/blob/main/LICENSE).

## Buy Me A Coffee

If this project has helped you in any way, do buy me a coffee so I can continue to build more of such projects in the future and share them with the community!

<a href="https://buymeacoffee.com/bonsaibauer" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/default-orange.png" alt="Buy Me A Coffee" height="41" width="174"></a>
