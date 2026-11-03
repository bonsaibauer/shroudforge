---
name: change-updater-queue-operation
description: Add or change a concrete updater queue operation across requests, selection, worker execution, progress and cancellation. Use when queue lifecycle behavior changes rather than for a new helper or a diagnostic job.
---

# Change an Updater Queue Operation

Read `.agents/skills/repository/repository-working-rules.md` and trace the requested operation through `modules/updater/lib.rs::run_request`, its owner request adapter, `queue/add_update_item.rs`, `queue/start_update_queue.rs`, `queue/run_update_queue.rs`, and `queue/update_queue_storage.rs`. Read the operation's business implementation fully before editing. UI and CLI adapters must call this owner rather than implement the operation again.

Define concrete input validation, stable item ID, kind/operation, payload, selection and required status, execution result, progress states and retry behavior. Update only relevant request/result adapters in `modules/modloader/ui/host/` and `modules/updater/ui/host/window.rs`, plus `modules/updater/ui/update-types.ts` and `show-update-queue.tsx` if the observable contract changes. Runtime control actions additionally use the declared owner set and claim/complete path under `modules/updater/control/`.

Preserve atomic queue storage and its lock, runId ownership, single-installation worker exclusion, required before-update migrations and after-install receipts. Uninstall runs exclusively. Do not let a new item bypass selected-item validation, active restore checks or game-exit waiting.

Trace `queue/cancel_update_queue.rs`, `queue/finish_cancelled_queue.rs`, `queue/set_update_item_state.rs` and `system-updates/check_update_cancelled.rs`. Preserve cancellable versus irreversible phases, pending cancellation, staged-file cleanup and rollback before reporting cancellation. A stale worker must not mutate the next run. Report failures through existing queue/status/activity paths with no duplicate completion event.

Use temporary fixture installations. Existing tests include cancellation/run-token behavior in `cancel_update_queue.rs` and `check_update_cancelled.rs`, migration tests in `maintenance_jobs.rs`, and transaction tests in `apply_release_files.rs`. Run the relevant `cargo test -p shroudforge-updater` filters and existing build checks for changed Windows/UI surfaces. Verify success, repeated start, busy installation, stale run, failure/retry and cancellation. Authoring this change does not authorize changing a live installation.
