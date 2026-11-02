---
name: add-world-editor-action
description: Add or change a World Editor control or command through the native window action queue, Lua owner, state/progress publication, and local or dedicated-server execution path.
---

# Add a World Editor action

Read `.agents/skills/repository/repository-working-rules.md`. Identify the user action, inputs, local/server behavior, state/progress result, and mutation/undo requirements. The native UI supplies inputs and displays results. The Lua mod owns blueprint/world behavior.

## Trace the existing end-to-end path

1. `modules/world-editor/ui/index.html` sends a command. `ui/parse_editor_message.rs` parses it and `ui/window.rs` enforces the enabled module.
2. `ui/run_blueprint_action.rs` handles local screenshot actions or calls `queue_editor_action.rs::dispatch_action`, which atomically publishes action JSON in the configured UI actions directory.
3. `modules/mod-runtime/lua/shroudforge/v1/dispatch_mod_ui_actions.rs` consumes the action. `mods/world-editor/src/mod.lua` registers its `shroudforge.ui.on_action` callback.
4. Lua owners are `operations.lua` for canonical capture/paste, `history.lua` for inverse journals/recovery, `blueprint.lua` for format/coordinates, `library.lua` for persistent exports, and `preview.lua` for cursor/preview/progress/state.
5. State returns from `preview.lua` through the configured `world-editor/editor-state.txt` export to `modules/world-editor/blueprint_library.rs::view_state`, visible-only `ui/watch_editor_state.rs`, `ui/window.rs`, and `__worldEditorUpdate`.

Read the complete affected owners, manifest, callbacks, fallback renderer, and tests before editing. `ui/fallback.rs` is a current consumer of the UI state/action contract.

## Implement the narrow action

Add only required package settings/groups/actions/keybinds in `mods/world-editor/extended.mod.json`. Use declared SDL bindings and the existing key label/edge detection path. Add world behavior once in the matching Lua owner and register the action in `mod.lua`. Native screenshot/file actions stay in their existing Rust UI owner.

For local/server world work, select the backend from validated native world context. Unknown context sends no write/request. Pass remote validated inputs into the same canonical capture/paste/undo. Do not repeat feature/grid checks in server coordinators or require a scale writer for unit-scale props. Preserve bounded pre-dispatch waits, session fencing, expected-value/conflict checks, retained partial journals, peer isolation, and no retry after uncertain mutation.

Update published state, Rust parser/serde state, primary renderer, and existing fallback only where the action needs them. Keep progress honest, including no-change, unavailable, pending, partial, and confirmed outcomes. Native handles stay process-local. Live readback and persistent world-save behavior are distinct.

## Verify

Extend the meaningful shipped Lua fixture in `modules/mod-runtime/tests/world_editor_lifecycle.lua` and run `cargo test -p shroudforge-api --test bundled_lua`. Rebuild the Rust product for embedded UI changes with `./dev/build/build_modloader.ps1`. Exercise the requested actual local/server action, UI state, undo/failure recovery, and logs when live checking is part of the task. The UI uses its existing private WebView profile, focus/visibility policy, owner/stop lifetime, and hidden-window scan suppression. Preserve those boundaries.
