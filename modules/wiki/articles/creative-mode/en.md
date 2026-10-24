# Multi DLL Manager

> 🧪 **BETA**

ShroudForge detects Phoenix1355's Creative Mode automatically when its mod folder is in `mods`. You do not need to edit `mod.json` or set up its DLL manually in DLL Manager.

## Installing Creative Mode

1. Download Creative Mode from [Nexus Mods](https://www.nexusmods.com/enshrouded/mods/146) and extract the archive.
2. Put the Creative Mode folder in ShroudForge's `mods` folder. ShroudForge detects the mod automatically.

## Opening Creative Mode in game

- Press **Insert** to open or close the Creative menu.
- Press **G** to toggle character flight.
- In the menu, choose items and add them to your inventory or an action bar.
- Find mod settings and key bindings in the Creative menu's Settings tab.

## Other mods with DirectInput DLLs

For other mods that include a DirectInput DLL, usually named `dinput8.dll`, the recommended location is that mod's own folder under `mods/`. Keep the original filename. See the existing [DLL Manager Wiki article](#wiki/dll-manager) for DLL variants.

## Compatibility settings

For other mods that need a DLL to start, open **Compatibility → DLL Manager** in the Modloader. There you can:

- enable or disable the DLL Manager globally and toggle loading for each mod
- choose automatic matching or a specific compatibility profile
- select entry DLLs and their startup method for a manual selection
- change the order of selected DLLs
- choose the game client, game server or both as the load target
- explicitly approve a manual DLL selection
- exclude DLL filenames from detection and loading
- review existing `dinput8-chain.ini` entries and loading status

Compatibility settings do not change installed mod files. DLL loading changes apply the next time the game starts. A “DLL loaded” status confirms the loading step, not every mod feature.

## More Wiki articles

- [DLL Manager](#wiki/dll-manager)
- [Mod detection](#wiki/mod-detection)
- [Download Creative Mode](https://www.nexusmods.com/enshrouded/mods/146)
