---
name: add-mod-ui-action
description: Add a package-declared Modloader button and its Lua shroudforge.ui.on_action callback, using the existing queue, runtime activation, localization, and result-reporting contracts.
---

# Add a mod UI action

Read `.agents/skills/repository/repository-working-rules.md`. Identify the package, action ID, requested effect, applicable runtime target/phase, current settings, and observable result. A package action is different from a new native host command or a World Editor window action.

## Use the actual dynamic path

- Declare the action in an existing `groups[].actions[]` entry of `mods/<mod-id>/extended.mod.json`, with the current schema fields such as `id`, `label`, `style`, and optional `confirm`.
- `modules/modloader/ui/run-mod-action.tsx::ModActionButton` sends `run-mod-action` after its declared confirmation. Existing host parsing/window dispatch reaches `ui/host/request_mod_action.rs` and `queue_mod_action.rs`.
- The queue writer resolves the real package, validates identifiers and manifest declaration, then writes action JSON into the configured UI actions directory. Do not add another button/action registry or filesystem queue.
- `modules/mod-runtime/ingame/update_ingame_runtime.rs` calls `dispatch_ui_actions` before normal callback updates. `lua/shroudforge/v1/dispatch_mod_ui_actions.rs` resolves the action only for an active runtime mod.
- `lua/shroudforge/v1/create_mod_api.rs` implements `shroudforge.ui.on_action` with a named registry key scoped by mod ID and action. Register the callback in the package's current Lua entrypoint or its owned action module.

Read the complete touched package, host, dispatcher, schema, and lifecycle consumers. Preserve valid identifier rules and match the declaration and callback ID exactly. Ordinary Modloader buttons supply no payload. The dispatcher also supports a string payload from existing specialized callers, so do not silently change either contract.

## Implement the package action

1. Add the declaration to the appropriate existing group without changing unrelated settings or package format. Preserve activation, capabilities, dependencies, and client/server targets. The active runtime plan and checked APIs enforce actual execution eligibility. Do not bypass them because a request was queued.
2. Register `shroudforge.ui.on_action(actionId, callback)` through the existing API. Read current applied setting values inside the callback where live changes matter. Delegate behavior to the mod's canonical action implementation, including its existing world/session/ownership checks.
3. Keep runtime-native mutations within checked public operations and reversible ownership when appropriate. Queueing is not permission for arbitrary native calls, filesystem access, or a new lifecycle.
4. Localize label/confirmation through the package's current `locales/<locale>.json` catalogs when the package uses them. `modules/modloader/resolve_mod_locale.rs` validates action IDs inside the corresponding source group by array order and keeps source text as fallback. Translate Lua-facing result strings through `shroudforge.i18n.get` if needed. Do not add a second locale format or reorder groups without updating their catalogs.
5. Preserve error/result meaning. The host's action-queued response proves queue acceptance only. Runtime callback errors are logged by the dispatcher. Report completion/effect through the package's existing logging/notification/progress mechanism, without claiming a queued button already succeeded.

The dispatcher removes malformed, inactive, or completed/failed requests. It retains requests waiting for callback registration and consumes bounded batches. Preserve that behavior and the shared `invoke_mod_action` path used by diagnostic callers.

## Verify

Use current manifest/locale checks, Lua compilation, and meaningful callback/dispatch tests. Existing dispatcher tests cover absent/text payloads, inactive/invalid/unregistered callbacks, errors, and queue retention. Run relevant package/runtime tests and `cargo test -p shroudforge-api --test bundled_lua`. Rebuild the frontend/product only when those owners changed. If requested, activate the intended client/server runtime, press the actual button, inspect current logs/result, and verify the effect. Report queue acceptance, callback execution, and confirmed effect separately.
