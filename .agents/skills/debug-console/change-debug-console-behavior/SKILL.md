---
name: change-debug-console-behavior
description: Change Debug Console log sources, filtering, alert policy, preferences, or native-window lifecycle through the existing bounded readers and shared configuration/window-state contracts.
---

# Change Debug Console behavior

Read `.agents/skills/repository/repository-working-rules.md`. Establish the requested log/display/alert/window behavior and follow the actual startup or desktop caller. Debug Console is a linked module of `shroudforge.exe`, not a separate frontend bundle.

## Read the owners and callers

The canonical module is `modules/debug-console/`, crate `shroudforge-debug-console`.

- `read_log_tail.rs`: bounded tails and partial leading-line handling.
- `scan_log_for_alert.rs`: incremental current-session scan, log truncation, popup modes, bootstrap-ready boundary.
- `save_console_preferences.rs`: recognized preference fields through the canonical loader updater.
- `ui/read_console_snapshot.rs`: current log paths/tails and display preferences.
- `ui/window.rs` and its helpers: configuration refresh, visibility requests, owner/game/stop/show handles, SDL toggle debouncer, focus/topmost, cooldown, fatal-exit review deadline, and visible-only refresh.
- `ui/app.ts`, `index.html`, `styles.css`, `render_console_document.rs`: embedded document, search/filter/pause/follow/copy display.

Startup comes from `modules/entrypoints/windows-bootstrap/module_processes.cpp::start_debug_console`, which supplies root/PID/events/log offset/startup-watch and skips interactive console startup for dedicated servers. Desktop comes from `modules/modloader/ui/host/request_module_visibility.rs` with an owner PID. `modules/entrypoints/loader/process_modes.rs` dispatches `--debug-console` to `run_module`.

## Implement through the existing contracts

1. Keep log reading separate from presentation. Use `shroudforge_loader_config::paths` for loader logs and the existing game log source. Preserve bounds, read states, UTF-8 behavior, truncation handling, and current-session offsets.
2. Preserve logger format/level semantics in alert detection and UI filtering. Display filtering and automatic alert policy are different decisions. Do not widen bootstrap-only popup behavior to unrelated logs without a requested change.
3. Store supported preferences through the canonical loader configuration. If a new setting is required, trace `modules/loader-config/loader.default.json`, `loader.schema.json`, module `ui/settings.ts`, and its real settings consumer. Do not create another console preference file.
4. Preserve window request/publish IDs, module enabled behavior, SDL capture/focus rules, popup cooldown suppression after hide, owner/game exit cleanup, and private WebView profile. Hidden windows must not keep performing expensive display refreshes.
5. Keep HTML/JS/CSS embedded through `render_console_document.rs`. This window is not built by Vite. Rebuild Rust when embedded document content changes.

## Verify

Use existing focused reader/alert/config checks and meaningful fixtures for changed boundary behavior. Build the owning crate/product with the existing developer actions. For a requested live check, verify the relevant startup/desktop mode, real log path/tail, selected filter/pause/follow behavior, alert/hide/cooldown, visibility toggle, and process exit using current logs. Preserve server file logging. Report checks actually performed and separate render success, alert detection, and process lifetime behavior.
