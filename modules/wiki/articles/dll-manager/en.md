# DLL Manager

In the Modloader, open **Compatibility → DLL Manager**. The guide is shown at the top of that panel. It lists the mod DLLs ShroudForge found and whether they loaded. Known mods have prepared profiles. For other DLLs, select and confirm them there.

## Can I use more than one native DLL mod?

Yes. Install each mod in its own folder directly under `mods/`, and keep the files and subfolders supplied by the author. A matching profile may already provide the right DLL selection. For a mod without a matching profile, open **Adjust DLL selection** in the DLL Manager. Select only the entry DLLs named by the author. Choose the load target and confirm that you trust the DLL. Keep **Normal DLL** for ordinary native libraries. Choose **DirectInput proxy** only for a DLL intended to provide DirectInput that exports `DirectInput8Create`. Click **Apply selection**, then restart the game. Detection alone does not load an unprofiled DLL. It needs a confirmed DLL Manager selection, a declaration in `native-plugin.ini` or `dinput8-chain.ini`, or registration by its Lua mod.

## What should I do with a downloaded mod?

- **No DLL included:** Install the mod as usual. Mods made for ShroudForge or EML do not need an extra native DLL.
- **DLL included:** Put the mod DLL in its mod folder as shown below. It then appears in the DLL Manager. A known profile already has its selection prepared. Otherwise, select the DLL there and confirm it.
- **Other mod files included:** Keep the complete mod folder. Do not remove files such as `mod.json`, `native-plugin.ini` or configuration files.

## If one mod contains several DLL files

Many mod folders contain DLLs that are only helper files. Select only the DLLs named for loading by the mod profile or the mod author's instructions. The up and down controls set the order of selected DLLs within that mod folder. They do not set a global manual order for ordinary DLLs across separate mod folders.

## Installing a mod DLL

If the ShroudForge installer recognizes the downloaded mod package, install it there. For a manual install, give each mod its own folder directly under `mods/` and put the mod's DLL in that folder. If the DLL sits beside the mod folder in the archive, put it with the mod. Keep the original filename and the package's subfolders.

```text
mods/
  my-mod/
    mod.json        (if included)
    dinput8.dll     (or the mod's DLL)
    config.ini      (if included)
```

A DLL does not load just because it is in the folder. Select it in the DLL Manager as described above.

## Why can the mod file also be named dinput8.dll?

ShroudForge's own startup file is `dinput8.dll` in the game folder. The mod file is separate at `mods/<mod-folder>/dinput8.dll`. ShroudForge loads it from that full path. With a matching profile or DLL Manager selection, ShroudForge forwards DirectInput calls to it. Both files can keep the same name. Do not rename the mod file or put it in the game folder. Do not replace ShroudForge's startup file there.

## Understanding profiles

A profile is a prepared setting for a known mod. It tells ShroudForge which files to load and how. ShroudForge matches it by the mod ID in `mod.json`, or by the folder name if there is no ID. You can also choose a profile in the DLL Manager. Profiles do not change mod files. A saved custom DLL selection replaces the profile's file selection.

## Existing DirectInput chain

An existing game-folder `dinput8-chain.ini` uses the established format:

```ini
[Chain]
mods\my-mod\dinput8.dll
mods\another-mod\native.dll
```

Entries must resolve to files inside direct mod packages such as `mods/<mod-folder>/`. Game-root DLLs and paths outside those packages are rejected. Their order takes priority over automatically derived declarations. This file sets order for the DirectInput provider chain, not for ordinary DLLs across mod folders. The same resolved file path is loaded only once. The first valid DLL exporting its own `DirectInput8Create` receives forwarded calls. Later DirectInput proxy DLLs may still load, but their DirectInput exports are not active in the chain. Do not assume that multiple proxies will work together. Own startup files are skipped, and recursive forwarding falls back to the original Windows library.

## Switches, process target and restarting

The main switch controls all native loading performed by ShroudForge, including INI, profile, chain and Lua registration. A disabled selection suppresses automatic loading for that folder. Client, server or both determines the target process. Filename exclusions apply immediately before loading. They do not identify a mod's functionality from its name.

Loaded DLLs remain in memory until the process exits. Disabling them, changing the order or choosing another profile therefore requires restarting the game. Duplicate declarations of one file do not create another instance.

## A loading error appears

Check the mod name, file path and cause. Windows error 126 can mean a missing main file or one of its required libraries. If the DirectInput export is missing, check the selected file and its loading mode. “Loaded” confirms loading, rather than every feature of the mod.

An unknown native plugin format, Vulkan layer, additional complete loader or external program needs its intended integration. Loading a library alone does not replace that interface.

## Installing packages

The installer also finds a single mod package in nested archive folders. Known profiles import explicitly named companion files. A fresh archive containing several mod packages is not installed automatically as a group. Extract it and install one mod folder at a time. Normal runtime operation does not open archives or start external programs.
