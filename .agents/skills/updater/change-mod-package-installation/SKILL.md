---
name: change-mod-package-installation
description: Change downloaded mod archive selection, extraction, validation, staging or recoverable replacement while preserving runtime unload/reload and registry consistency. Use for the updater's installation transaction, not normal unpacked mod discovery.
---

# Change Mod Package Installation

Read `.agents/skills/repository/repository-working-rules.md`, `modules/updater/mod-installation/requests.rs`, `download_mod_package.rs`, `extract_mod_archive.rs`, `verify_mod_checksum.rs`, `stage_mod_in_storage.rs`, `install_catalog_mod.rs`, `replace_installed_mod.rs`, `catalog.rs`, and their callers in `modules/updater/queue/run_update_queue.rs`. Follow the real package reader in `modules/modloader/read_mod_manifest.rs` and data-only compatibility adapters in `mod_profiles.rs`.

Preserve one selected manifest per archive, expected mod identity during updates, valid Lua/native entrypoint checks, required adapter files and bounded package-relative `importFiles` lookup. Keep traversal/symlink rejection, compressed/expanded size and entry-count limits, HTTPS/hash requirements and cancellation checks. Unsupported native-only archives must not silently become executable packages.

Stage files through `stage_mod_in_storage.rs` on the mods storage volume so final rename remains valid across separate download/storage drives. Installed mods are unpacked directories. Do not relocate archive reading into the running Modloader.

For replacement, preserve the complete sequence: validate new package, unload the affected runtime/dependents, create the verified recoverable previous-package entry, commit the new folder, reload, then update the existing catalog registry. Follow every failure branch through restoration/reload and registry recovery. `modules/updater/recycle_bin.rs` owns recoverable storage. Do not introduce another backup format or discard unknown package data before successful commit.

Verify temporary fixture installations for initial install, update, multiple/no manifest, identity mismatch, missing companions, cross-volume staging, failed unload/reload, failed commit, cancellation and rollback. Use existing `cargo test -p shroudforge-updater` checks plus relevant `shroudforge-package` manifest/profile checks, and add focused transaction coverage where behavior changes. Do not use installed user mods as disposable fixtures. Report which runtime failures were simulated and which were actually exercised.
