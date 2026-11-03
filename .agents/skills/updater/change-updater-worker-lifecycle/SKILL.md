---
name: change-updater-worker-lifecycle
description: Change Windows updater handoff, independent worker copies, process waiting, file locks or shutdown cleanup while preserving per-installation queue ownership. Use for updater process lifecycle and self-replacement behavior.
---

# Change the Windows Updater Worker Lifecycle

Read `.agents/skills/repository/repository-working-rules.md`, `modules/updater/bin.rs`, `windows/entry.rs`, `windows/worker.rs`, `windows/scheduler.rs`, `windows/restore_gamefiles.rs`, queue start/run/cancellation, and both hosts using the affected action. This code owns Windows process behavior. UI and request adapters call the owner.

Trace the exact mode and arguments: request, queue, update worker, standalone window, original-gamefile restoration or explicit complete-data purge. Installed executables hand managed long-running modes to a unique independent updater copy, marked `--ephemeral-worker`, so release installation can replace the installed updater. Recycle restoration can also replace it and therefore has its own request handoff and parent wait. Preserve stdin/result forwarding and failure cleanup.

Preserve installation-scoped named mutex ownership and runId checks. Game matching canonicalizes installation/image paths, handles Steam junctions and does not assume an inaccessible process has exited. Respect cancellation during waits. File-lock handling distinguishes owned ShroudForge processes from foreign processes. Regular release updates request graceful close and report remaining locks. Forced owned-process shutdown is confined to the explicit complete-data purge path.

Retain copy verification, unique worker filenames, stale-worker age and process checks, hidden process creation and close-after-install behavior. Purge workers must stay outside the removed tree, wait for their parent, validate actual game root and data-directory/reparse/temp boundaries, and use the existing cleanup path. Never broaden deletion targets or kill unrelated processes as a workaround for a locked file.

Use isolated temporary installation fixtures and controlled helper processes for lifecycle verification. Existing host close/queue tests are in `modules/updater/ui/host/window.rs`; cancellation tests are in queue and system-update modules. Run relevant `cargo test -p shroudforge-updater` checks and the canonical Windows build. Record actual mode/arguments, process ownership evidence, observed status/logs, and limits of mocked tests. Do not terminate, install into or purge the user's live installation merely to test authoring changes.
