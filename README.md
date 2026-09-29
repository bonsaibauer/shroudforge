# ShroudForge

[![Made With Love](https://img.shields.io/badge/Made%20with%20%E2%9D%A4%EF%B8%8F-by%20bonsaibauer-green)](https://github.com/bonsaibauer)
[![Repository](https://img.shields.io/badge/Repository-shroudforge-blue?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge)
[![License](https://img.shields.io/badge/License-MIT-blue)](https://github.com/bonsaibauer/shroudforge/blob/main/LICENSE)
[![Visitors](https://visitor-badge.laobi.icu/badge?page_id=bonsaibauer.shroudforge)](https://github.com/bonsaibauer/shroudforge)
[![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?style=flat&logo=windows&logoColor=white)](https://github.com/bonsaibauer/shroudforge)


[![Report Bug](https://img.shields.io/badge/Report-Bug-critical?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge/issues/new?template=bug_report.yml)
[![Request Feature](https://img.shields.io/badge/Request-Feature-green?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge/issues/new?template=feature_request.yml)
[![Compatibility Problem](https://img.shields.io/badge/Report-Compatibility_Problem-important?style=flat&logo=github)](https://github.com/bonsaibauer/shroudforge/issues/new?template=version_mismatch.yml)

![GitHub Stars](https://img.shields.io/github/stars/bonsaibauer/shroudforge?style=social)
![GitHub Forks](https://img.shields.io/github/forks/bonsaibauer/shroudforge?style=social)

## Welcome!

ShroudForge helps you install and make mods for **Enshrouded**. A mod is a small add-on that changes or adds something in the game. You can use mods made by other players, or create your own.

New to mods? Start with the illustrated guide. It explains everything in simple steps, in English and German:

- [English: install ShroudForge, add mods, and make your first mod](https://bonsaibauer.github.io/shroudforge/en/)
- [Deutsch: ShroudForge einrichten, Mods hinzufügen und den ersten Mod bauen](https://bonsaibauer.github.io/shroudforge/de/)

The guide includes separate instructions for your PC and game server, pictures of the Modloader, a first-mod lesson, and an interactive preview for mod settings.

![ShroudForge Modloader in Enshrouded](assets/modloader-ui-2.png)

## Quickstart: use ShroudForge

1. Close Enshrouded.
2. Download the Latest release:
   
   [![Latest release](https://img.shields.io/github/v/release/bonsaibauer/shroudforge?label=Latest%20Release)](https://github.com/bonsaibauer/shroudforge/releases/latest) [![Downloads](https://img.shields.io/github/downloads/bonsaibauer/shroudforge/total)](https://github.com/bonsaibauer/shroudforge/releases)
   
3. Right-click the downloaded ZIP and choose **Extract All**.
4. Open the Enshrouded folder that contains **enshrouded.exe**.
5. Copy the files from the ZIP into that folder, beside **enshrouded.exe**.
6. Start Enshrouded and press **F9** to open the Modloader.

A common Steam folder is:

~~~text
C:\Program Files (x86)\Steam\steamapps\common\Enshrouded
~~~

The Modloader is the ShroudForge window inside the game. It shows your mods and lets you switch them on or off. Press **F10** to read messages if something does not work.

### Add a mod

Follow the mod creator's instructions. Unpack the mod download, then copy the mod's folder into **mods**, beside **enshrouded.exe**. Its **mod.json** file should be directly inside the mod folder:

~~~text
Enshrouded/
├── enshrouded.exe
└── mods/
    └── my-mod/
        ├── mod.json
        └── src/
            └── mod.lua
~~~

Open the game, press **F9**, find the mod, and switch it on. Some mods need a game restart. Mods that change game files must prepare those changes before the game starts.

Read the full [PC quickstart](https://bonsaibauer.github.io/shroudforge/en/#play) or [server guide](https://bonsaibauer.github.io/shroudforge/en/#server) for screenshots and help with each step.

## Make your first mod

Start with the [mod template](templates/mod/README.md). It gives you the files for a small mod and explains what to change.

The website's [first-mod lesson](https://bonsaibauer.github.io/shroudforge/en/#first) walks through a complete example. When you are ready, try the [mod settings guide](https://bonsaibauer.github.io/shroudforge/en/#manifests): edit an example on the left and see the Modloader-style preview on the right. It also shows how `extended.mod.json` exposes a setting and how Lua reads it with `shroudforge.settings.get`.

To look up functions or Enshrouded game information, open the [searchable API reference](https://bonsaibauer.github.io/shroudforge/en/#api).

## Mods included in the release

The build checks mod folders for **mod.json**, a mod ID, and **src/mod.lua**, then checks the Lua files before packaging them. These are the 17 packages included in the release build today:

| Mod | What it does | Status |
| --- | --- | --- |
| **Fishing Data Exporter** | Exports fishing data and English item names to CSV files for community research. | 🚧 Beta |
| **Item Exporter and English Translator** | Exports item information and English names to CSV files. | 🚧 Beta |
| **KFC Parser Mimic** | Exports Enshrouded game data in a format similar to the KFC Parser tool. | 🚧 Beta |
| **SF Infinite Item Split** | Keeps the selected amount from being removed from the original stack when splitting items. | ✅ Proven |
| **SF Infinite Item Use** | Prevents item use from consuming the item. | ✅ Proven |
| **SF No Fall Damage** | Prevents fall damage. | ✅ Proven |
| **SF No Resource Cost** | Removes supported crafting costs. | ✅ Proven |
| **SF No Stamina Loss** | Prevents stamina from running out. | ✅ Proven |
| **SF Unlimited Flight** | Applies the supported change to flight. | ✅ Proven |
| **SF Unlock Blueprints** | Unlocks supported crafting recipes. | ✅ Proven |
| **World Editor** | Adds shortcuts for marking areas, undoing, and saving or placing blueprints. | ✅ Proven |
| **Auto Loot** | Automatically collects nearby harvest drops that match the configured whitelist. | 🚧 Beta |
| **2x Grappling Hook Pull Distance** | Doubles grappling hook pull distance; swing distance stays unchanged. | 🚧 Beta |
| **Item Stack Limit 65535** | Raises the stack limit of stackable items to 65,535. | 🚧 Beta |
| **Unlimited Flame Altars** | Removes the Flame Altar limit. | 🚧 Beta |
| **Vein Mining** | Mines matching ore in an area around the hit point. | 🚧 Beta |
| **20x Workshop Production Speed** | Increases timed workshop recipe production speed by 20×. | 🚧 Beta |

## Development roadmap

This table describes the current state of ShroudForge itself. **Proven** means the workflow is in place for the supported build; **Beta** means it exists but still has known reliability or coverage limits; **Open** means more implementation or validation is needed. A mod being included in the release does not make every runtime feature it uses proven.

| Area | Current state | Status |
| --- | --- | --- |
| **Release build and mod checks** | Release packaging checks each mod's manifest, ID, Lua entry point, and Lua syntax. | ✅ Proven |
| **Build profiles** | Profiles are kept per Enshrouded build and target. The current profile is for client build **1076226**. Supporting another game build usually means reviewing and updating its profile; mods do not need per-build copies unless game behavior or the mod API changes. | ✅ Proven |
| **Asset changes before launch** | Supported asset edits are prepared before the game starts, so changes take effect on the next launch. Coverage and compatibility still depend on the resource and game build. | ✅ Proven |
| **KFC runtime and ECS** | The native runtime and ECS API are present, but live ECS discovery, queries, reads, and writes are not reliable enough to treat as a stable foundation yet. Keep this path experimental while it is being corrected. | 🚧 Beta |
| **Runtime hooks and patches** | Build-profiled hooks and guarded runtime patches provide the current route for many live gameplay changes. They depend on executable signatures and game behavior, so a game update can make individual operations unavailable or require new native work. | 🚧 Beta |
| **Current gameplay mods** | Several runtime mods rely on targeted hooks or runtime patches while the ECS path is incomplete. Their status is specific to the supported build and does not guarantee compatibility with every game update or mod combination. | 🚧 Beta |
| **EML mod support** | ShroudForge reads EML-style packages and implements EML v1 APIs, including supported export, asset-patch, runtime, and package-local DLL flows. Compatibility depends on which APIs and native behavior an individual EML mod uses. | 🚧 Beta |
| **Mod discovery and updater** | The Modloader can find and install catalog mods, queue mod updates, and stage ShroudForge updates for after the game closes. System updates verify the download and keep a backup for rollback; the complete range of release and recovery scenarios still needs broader validation. | 🚧 Beta |
| **Runtime diagnostics** | Bounded runtime snapshots, mod activity, callback timings, and logs are available to help investigate failures. Diagnostics report observations; they do not certify that a game operation or mod is safe. | 🚧 Beta |
| **More game builds and server profiles** | Add and validate profiles for further Enshrouded builds and targets, with live checks for the operations each profile enables. | 🕓 Open |
| **Stable ECS-backed mod API** | Make ECS discovery and access dependable, verify gameplay effects in live sessions, and document which operations are supported before moving mods away from their current hooks and patches. | 🕓 Open |

## What is inside the modules folder?

The folder **src/loader/modules/** contains five built-in Rust modules. They are parts of ShroudForge; they are not separate mods you need to install.

| Module | What it does |
| --- | --- |
| **Commands** | Provides a command-module entry point and reports whether it is available. |
| **Debug Console** | Shows and filters messages from Enshrouded and ShroudForge. Press **F10** in game. |
| **Modloader UI** | Lets you find, install, manage, and update mods. It also shows settings, notices, and compatibility details. Press **F9** in game. |
| **Runtime Diagnostics** | An optional, time-limited view of runtime health and mod activity. |
| **Updater** | Queues mod install and update work, and applies ShroudForge updates after Enshrouded closes. |

There is also **ui-shared**, a small folder of shared visual styles used by the user interfaces. It is not a separate running module.

The loader also has major parts outside this folder, including its API, mod package handling, compatibility checks, parser, and startup workflow. See the [developer architecture guide](docs/sf/Architecture.md) for the full map.

## For contributors

The [developer documentation](docs/sf/README.md) explains how the repository is organized and where to find the mod, runtime, parser, and Modloader content guides.

## Help and community

If something goes wrong, check the [guide](https://bonsaibauer.github.io/shroudforge/en/) and the mod creator's instructions first. If you still need help, [open an issue](https://github.com/bonsaibauer/shroudforge/issues/new). Tell us what you expected, what happened, and which game, ShroudForge, and mod versions you use. The **F10** messages can help explain the problem. Please remove private information before sharing them.

- [Report a bug](https://github.com/bonsaibauer/shroudforge/issues/new?template=bug_report.yml)
- [Suggest a feature](https://github.com/bonsaibauer/shroudforge/issues/new?template=feature_request.yml)
- [Report a compatibility problem](https://github.com/bonsaibauer/shroudforge/issues/new?template=version_mismatch.yml)

## License and support

ShroudForge is shared under the [MIT License](https://github.com/bonsaibauer/shroudforge/blob/main/LICENSE).

# Buy Me A Coffee

If this project has helped you in any way, do buy me a coffee so I can continue to build more of such projects in the future and share them with the community!

<a href="https://buymeacoffee.com/bonsaibauer" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/default-orange.png" alt="Buy Me A Coffee" height="41" width="174"></a>
