---
name: change-system-installation-transaction
description: Change system release staging, installation, repair, uninstall or restoration while preserving managed paths, recoverable backups, cancellation and rollback. Use for release-file transactions rather than update detection or mod package replacement.
---

# Change a System Installation Transaction

Read `.agents/skills/repository/repository-working-rules.md`, the owner request path in `modules/updater/lib.rs`, `system-updates/requests.rs`, `queue_installation.rs`, queue execution, and every affected transaction file: `stage_system_update.rs`, `install_system_update.rs`, `apply_release_files.rs`, `read_managed_release_files.rs`, `validate_update_paths.rs`, `repair.rs`, `uninstall_system.rs`, `restore_release_files.rs` and `modules/updater/recycle_bin.rs`. Trace `dev/build/package_release.ps1` as the writer of the release manifest consumed here.

Preserve HTTPS/SHA-256 verification, archive bounds, manifest/release identity checks, protected game/user paths, allowed managed paths, staging promotion and pending.ready semantics. The release package owns program files. User configuration, generated state, locks and unrelated game files must not be reclassified as disposable managed files. Historical game-build profiles remain immutable references in the source repository.

Wait for the intended installation's game/bootstrap processes and release locks through the existing Windows scheduler. Preserve a verified recoverable backup before applying changed/obsolete paths. Apply the version manifest last and atomically. On failure or cancellation, complete rollback before returning a terminal result and retain useful recovery evidence. Keep queue run tokens, post-install receipts, status, activity and cached checked-release state consistent.

Uninstall options explicitly distinguish program removal, mod removal, settings reset and complete-data purge. Keep verified-original gamefile restoration separate from release rollback. Reset preserves configured storage paths. Purge must use the existing independent worker and cannot become an implicit cleanup step for install/repair. UI request adapters retain their existing confirmation and option contract.

Use temporary fixture installations. Existing tests live in `apply_release_files.rs`, `install_system_update.rs`, `uninstall_system.rs`, `validate_update_paths.rs`, `check_update_cancelled.rs` and `recycle_bin.rs`. Run relevant `cargo test -p shroudforge-updater` filters and the normal Windows build when Windows-only code changes. Exercise interrupted copy, obsolete removal, version-last failure, foreign locks, cancellation, rollback and option preservation. Never run install/uninstall/purge against a live user installation just to author or verify this change.
