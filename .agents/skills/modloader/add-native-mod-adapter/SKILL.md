---
name: add-native-mod-adapter
description: Add a data-only compatibility adapter for an existing native mod, including manifest overlays, package-local DLL declarations and archive companion mappings. Use for modules/modloader/mod-profiles, not for game runtime build profiles or executing an unapproved DLL.
---

# Add a Native Mod Compatibility Adapter

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Read `modules/modloader/mod_profiles.rs`, `dll_manager.rs`, `read_mod_manifest.rs`, the existing adapter and the target package/archive layout. Confirm the exact package directory, optional manifest ID, process target, DLL paths and required companion files.

Create one descriptive JSON file under `modules/modloader/mod-profiles/`. Follow the current `ModCompatibilityProfile` serde contract and `creative.json`. The catalog discovers these files, while release packaging installs them under runtime profiles.

Use `modJson` and `extendedModJson` only for necessary compatibility overlays. Never replace the source ID or version. Existing code unions capabilities and ignores overlay activation so user choices remain authoritative. `packageFiles` are virtual package contents, not a reason to rewrite the installed package. Preserve the current relative-path, count and size limits.

Declare DLLs in `files` in their required order and mode. The one load plan in `dll_manager.rs` combines profiles, manual selections, native-plugin.ini and chain entries, applying activation, dependencies, conflicts, targets and exclusions. Reading an inventory must not load a DLL.

If an archive keeps companions outside its selected package, use the existing `importFiles` mapping. `modules/updater/mod-installation/download_mod_package.rs` imports those companions within the extraction root and verifies required profile DLLs. Archive handling stays in installation. The running loader consumes unpacked folders.

Verify adapter parsing, matching, overlays, resolved paths and packaging without executing the mod. Perform a live DLL load only when included in the user's authorization, respecting existing selection approval and process lifetime. Report static compatibility evidence separately from an observed successful native load.
