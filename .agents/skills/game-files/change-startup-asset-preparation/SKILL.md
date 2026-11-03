---
name: change-startup-asset-preparation
description: Change pregame mod asset preparation, baseline selection, staged KFC writes, or atomic game-file publication and recovery. Use for transaction infrastructure rather than ordinary Lua resource edits.
---

# Change startup asset preparation

Read `.agents/skills/repository/repository-working-rules.md`. Establish the exact preparation trigger, installation, input files, companion outputs, baseline provenance, and intended behavior. Trace both successful commit and interrupted/failing paths before editing.

## Canonical owners

- Host entry: `modules/entrypoints/loader/asset_preparation/prepare_startup_assets.rs`.
- Plan/run/cache: `modules/mod-runtime/startup/run_startup_mods.rs` and `startup/cache/`.
- Baseline/open: `startup/manage_asset_baseline.rs`, `startup/open_game_assets.rs`.
- Session staging: `modules/mod-runtime/session/create_mod_session.rs` and `asset_resources.rs`.
- Serialization/commit: `startup/commit_asset_changes.rs`.
- Complete set transaction: `modules/game-files/game_file_transaction.rs`.
- Immutable originals: `modules/game-files/originals/`.
- Capture/restore request/status/UI: `modules/game-files/{request_game_file_restore,read_game_file_status}.rs`, `ui/show-game-file-backups.tsx`, and the actual updater worker.

KFC parser/writer formats stay in the direct `kfc-parser/` submodule. The runtime coordinates typed changes and the game-files module owns publication. Do not create a second transaction owner.

## Make the narrow change

1. Preserve startup scheduling, dependency failure behavior, game/mod/config fingerprints, force-patch semantics, and cache writes only after success. Export capability alone must not schedule a runtime-only package during pregame.
2. Keep the distinction between mutable working `.bak` baselines and verified immutable original backups. Preserve executable/hash and KFC3/KFC4 compatibility, companion files, and uncertainty checks. Do not delete or overwrite originals to repair an unrelated preparation failure.
3. Continue staging writers privately. Serialize only changed cached resources, finalize all required writers, then publish the complete accepted file set through the journaled transaction.
4. Preserve accepted path/name checks, game index-last publication, synchronization, rollback, interrupted recovery, and abort/drop cleanup. A partial companion set must not become the active game data.
5. Use configured storage/path resolvers and canonical status schemas. Capture/restore is routed through its updater owner. Preserve verification/safety copies and the current lightweight status refresh instead of adding expensive hashes on every UI poll.

## Verify

Use existing game-files transaction and runtime startup tests, adding a meaningful failure/recovery case for changed semantics. Run the affected crate tests and `cargo test -p shroudforge-api`. Build through existing `dev/build/` entrypoints. For authorized live preparation, record exact game executable identity, input/output set, preparation result, startup logs, and the requested game/server check. Keep backup mutation and restore within the user's concrete task and established ownership. Report unrun recovery/live checks honestly.
