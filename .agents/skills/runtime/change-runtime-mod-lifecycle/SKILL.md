---
name: change-runtime-mod-lifecycle
description: Change ShroudForge runtime mod activation, update scheduling, live settings, reload, unload, or retryable override cleanup while preserving dependency and scope ownership.
---

# Change runtime mod lifecycle

Read `.agents/skills/repository/repository-working-rules.md`. State the exact transition to change and trace it from the host entrypoint to status/configuration consumers. This skill changes runtime infrastructure. An ordinary package callback belongs in the mod.

## Read the transition owners

The canonical owner is `modules/mod-runtime/ingame/`:

- `plan_runtime_mods.rs` selects dependency-aware candidates and startup-preparation compatibility.
- `load_runtime_callbacks.rs` loads a lifecycle table/nil and validates callback/update-interval contracts.
- `activate_runtime_mod.rs` establishes the active writable scope before `on_load`.
- `start_ingame_runtime.rs`, `update_ingame_runtime.rs`, `update_mod_callbacks.rs`, `stop_ingame_runtime.rs` own host lifecycle and scheduling.
- `unload_runtime_mod.rs` and `unload_runtime_packages.rs` retain cleanup-pending scopes until callback and automatic override cleanup finish.
- `refresh_runtime_configuration.rs` owns canonical configuration polling and live activation/settings boundaries.
- `process_runtime_reload_request.rs`, `reload_runtime_packages.rs`, and `restore_runtime_packages.rs` own reload requests, replacements, and failure recovery.
- `publish_runtime_status.rs` and `session/record_runtime_state.rs` publish results for real consumers.

Also read the touched session structures, override registry, configuration writer/reader, native host caller, and lifecycle tests in full. Do not infer a transition from filenames alone.

## Preserve state and cleanup

1. Model the before/after transition and actual scopes/dependency ordering. Preserve disabled-but-live-activatable candidates and dependency blocking unless specifically changing them.
2. Keep active scope through startup callbacks and retryable teardown. Failed startup/update still needs cleanup. Do not dispose a mod while an unload callback or automatic override restoration retains unresolved ownership.
3. Preserve reverse-order unload, no missed-tick bursts, cleanup-only retries instead of reapplying normal effects, and interval validation.
4. For configuration, use the canonical package/configuration flow. Runtime-only activation, hybrid startup preparation, `applyAt: live`, restart-required fields, and changed definitions have distinct semantics. Do not silently promote restart settings to live behavior.
5. For reload, preserve request deduplication, configured request/result paths, replacement failure reporting, and restoration of the previous runtime. Native DLL handles remain process-owned and are not unloaded by Lua reload.
6. Update status consistently with what actually happened. Keep faults, cleanup-pending state, and current PID/session identity visible to consumers. Do not introduce another configuration/state store.

## Verify

Use meaningful transition regression coverage in `ingame/lifecycle_tests.rs` and relevant bundled Lua fixtures. Run `cargo test -p shroudforge-api`. Build the affected host/runtime through existing developer actions. When requested, exercise activation, changed settings, failed/retried unload, reload failure recovery, and game/server exit in the live target, retaining logs and distinguishing checks actually performed. No test success can substitute for a missing required live transition.
