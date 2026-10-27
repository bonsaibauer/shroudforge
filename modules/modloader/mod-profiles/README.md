# Mod compatibility profiles

The release build copies this directory to `shroudforge/runtime/profiles/mod-profiles`. Add a JSON file using the format below to extend the catalog. Files are read automatically, without a second registration list. Mod adapters and game-build profiles are separate catalogs.

```json
{
  "schemaVersion": 1,
  "id": "example-adapter",
  "modId": "example",
  "packageDirectory": "example",
  "name": "Example mod",
  "target": "both",
  "modJson": {},
  "extendedModJson": { "links": { "website": "https://example.com" } },
  "packageFiles": {},
  "importFiles": {},
  "files": [{ "path": "example.dll", "mode": "library" }]
}
```

- `modId` matches the ID from `mod.json`. When there is no ID, `packageDirectory` matches the folder name. No version range or DLL hash is required.
- The DLL Manager can explicitly select a catalog profile for another folder. Automatic selection is the default. Selecting no adapter keeps ordinary DLL selection available.
- `modJson` and `extendedModJson` are in-memory overlays. They cannot replace the mod ID or version. Users control mod activation. Any `enabled` field in `extendedModJson` is ignored. Existing capabilities remain declared. Profile capabilities are added without duplicates rather than replacing the original list. `packageFiles` supplies virtual files only when an adapter actually needs them. None of these rules rewrites the installed mod folder.
- `files` declares ordered payload DLLs relative to the mod folder. It may be empty for metadata-only adapters. `library` loads an ordinary DLL and leaves its internal startup to that DLL. `dinput8` also offers its DirectInput export to ShroudForge's chain host. Creative uses ordinary `library` loading, with no extra initialization call, Lua stub, capability or INI declaration.
- `importFiles` is installer-only. It maps explicit companion files from the archive's enclosing game folder into the installed mod folder. Other root bootstrap files are not copied implicitly.

Ordinary user choices live in the existing DLL Manager settings (`modules.dllManager.profiles`). These entries store the selected adapter, target, activation, confirmation and ordered DLL files. They are settings, not another adapter catalog. Nonempty local file selections replace the adapter's DLL list. A disabled selection suppresses that folder's automatic native loading.

`native-plugin.ini` keeps its single-DLL format:

```ini
[Plugin]
Enabled=1
Dll=example.dll
```

The INI declaration requires no fake Lua entrypoint. Lua calls to `loader.runtime.register_dll` retain the existing EML phase, activation and package checks. Both routes use the same process executor and DLL Manager controls.

An existing game-root `dinput8-chain.ini` is read in its established format:

```ini
[Chain]
mods\xhl-native-core\xhl-native-core.dll
```

Explicit chain order takes precedence over automatically derived DLL declarations. The first valid DirectInput provider keeps forwarded calls. Own bootstrap files are skipped, recursive forwarding falls back to System32, and repeated canonical paths reuse one process handle. Chain entries must resolve to files inside a direct installed mod package, such as `mods/<folder>/`. Game-root DLLs and paths outside those packages are rejected. Per-folder declarations also support a configured external mods directory.

The module switch and editable filename exclusions apply immediately before every ShroudForge load. Filenames alone do not identify a DLL's purpose, so `dinput8.dll` and `dbghelp.dll` are not forced exclusions. An included EML or Flame loader is not a mod payload. Select actual mod DLLs or use a reviewed adapter instead of loading every DLL recursively.

Loaded native hooks remain resident until process exit. Changing a selection requires restarting the game. The UI reports loading, Windows load errors and restart requirements. Successful loading does not prove that every mod feature works. An unknown native ABI, Vulkan layer or external tool needs its appropriate integration rather than a generic load claim.

Logging uses the `shroudforge::dll_manager` target. INFO reports execution outcomes, WARN reports declarations needing attention, ERROR reports load failures, DEBUG shows resolution and provider decisions, TRACE shows inventory and shared registrations.
