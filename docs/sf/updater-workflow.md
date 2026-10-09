# ShroudForge Updater workflow

## Goal

The Modloader Updates page and the standalone `shroudforge-updater.exe` use the same queue, update state, labels, and actions. The standalone window is a second view of the same updater, not a separate update flow.

The executable and the window are different things: `shroudforge-updater.exe` starts workers and launches the compact window. The compact window is the same Modloader UI bundle, started from a copy of `shroudforge.exe` with `--updater-window`.

Opening the Modloader UI or updater executable must never start a download by itself. A download starts only after the user chooses update items and presses a start action.

## Entry points

| Entry point | Available action | Expected behavior |
| --- | --- | --- |
| In-game Modloader UI | **Nach Spielende herunterladen** | Add the selected update to the queue. Wait for Enshrouded to exit, then open the updater window and run the explicitly queued action. |
| Desktop Modloader UI | **Jetzt herunterladen** / **Nach Spielende herunterladen** | The first starts the selected downloads immediately. The second waits for Enshrouded to exit. |
| Standalone updater EXE | **Jetzt herunterladen** / **Nach Spielende herunterladen** | Launch the compact Modloader UI and show the same queue and choices. Merely opening the EXE does not download anything. |

The game-file installation step always waits until the game has exited, even if the user chose to download immediately.

## Queue selection

Each available system release or catalog mod update is one queue item. The UI shows its name, version, source, size when known, and current state. Users can select all items, clear the selection, or select individual items. Compatible available items may be selected initially for convenience, but nothing downloads until the user presses **Auswahl herunterladen**.

Starting the selection offers **Jetzt herunterladen** and **Nach Spielende herunterladen** where both are safe. In-game mode only offers the after-game action. Downloads run sequentially in a stable queue order; one failed or cancelled item does not silently discard the rest.

## Shared states and actions

Use the same state names and translations in both windows:

`Verfügbar → In Warteschlange → Wartet auf Spielende (optional) → Wird heruntergeladen → Wird geprüft → Bereit zur Installation → Wartet auf Spielende (optional) → Wird installiert → Abgeschlossen`

An item can also become `Abgebrochen` or `Fehlgeschlagen`. Retry keeps the item in the queue. Removing one item affects only that item. **Warteschlange leeren** removes all queued items. **Download abbrechen** stops the current transfer, clears its partial files, marks that item cancelled, and closes the standalone updater window when the action came from that window.

## UI consistency

- Keep item rows, progress, queue counts, selection controls, and action labels visually consistent between Modloader Updates and the standalone updater.
- The Modloader UI may provide navigation and release notes; the updater window may use a compact layout. Both read and change the same persisted queue and show the same per-item state.
- Keep download and installation progress distinct. A completed download is not described as installed until the installer has finished.
- Require confirmation before clearing multiple queued items. Cancelling one transfer must not clear unrelated items.

## Implementation notes

### Source files and call path

The React `UpdateQueuePanel` in `src/loader/modules/modloader-ui/ui/src/main.tsx` posts UI commands. `src/loader/modules/modloader-ui/src/main.rs` decodes those commands and calls the queue functions exported by `src/loader/modules/updater/src/lib.rs`. Queue persistence, worker launch, system download/staging and installation live in `src/loader/modules/updater/src/main.rs`.

`shroudforge-updater.exe` enters through `src/loader/modules/updater/src/bin.rs`. Its `run_module()` routes `--run-queue` to the worker. With no arguments it launches a copy of `shroudforge.exe` with `--module-ui --desktop --updater-window`; the main executable then displays the compact view from the same React bundle. A system queue item is installed by an `--update-worker`. A catalog mod item is handed to `shroudforge.exe --catalog-install-worker`.

The catalog install worker is currently implemented in `src/loader/modules/modloader-ui/src/main.rs`, even though the updater queue starts it. The [refactor master plan](refactor-masterplan.md) moves this implementation to the updater's `install_mod.rs` and gives UI, CLI and file-based administration one command path. It also combines the separate UI and headless GitHub release checks. The cutover defines one current CLI/job request; old argument aliases and handoff formats are removed, with no compatibility adapters.

The current implementation stores system releases and catalog mod operations as individual records in `updates/update-queue.json`. The updater processes an explicitly selected group sequentially. It currently writes each mod operation into a one-item worker request when that item starts. The refactor replaces those separate payload shapes with one current typed job request and removes the old payload writer and reader.

Opening the Modloader UI or updater executable does not start a download. In-game mode exposes only the after-game action. Desktop Modloader mode exposes both start choices and opens the compact updater window to show progress. When started from that compact view, the queue keeps the existing window open. Cancelling an active transfer preserves unrelated and not-yet-started queue entries and closes the compact window; clearing the whole queue is a separate confirmed action. Cancellation is unavailable during installation.
