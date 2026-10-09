# World Editor capture investigation, 2026-10-09

## Live evidence

Inspected running client PID 20788 without restarting or editing its world.
Non-invasive CDB stacks are saved under `build/world-editor-live-stacks.txt`;
the follow-up disassembly is `build/world-editor-live-scan.txt`.
The loaded provider was built at 05:28:04 local time and was not replaced.

- At session +51.657s capture started; +51.945s it yielded a pending prop scan.
- The saved UI state remained `saveState=saving`, `saveCompleted=0`,
  `savePhase=Capturing blueprint`. File export had not started.
- A sampled runtime thread (25640) was inside `VirtualQuery`, called from the
  native prop scanner and `KfcRuntimeWorldEntityQueryPropsInBounds`. Disassembly
  confirms this is the CurrentTransform component page validation.
  One sample identifies a costly path; it is not a complete CPU profile.
- At +172.421s the capture was cancelled after world context loss. The user
  confirmed leaving/re-entering the world during this attempt. A subsequent
  terrain-read failure therefore does not prove the scan caused context loss.
- Only one blueprint was present. Repeated thumbnail reads are a separate UI
  responsiveness issue, not established as the cause of this capture delay.

## History

`3755dbc` (Oct 1) moved destroy calls to handle-based removal and registered
material feedback with prop recipes. `e79faaf` (Oct 9) switched prop component
resolution from static profile indices to the resolved component registry.
`cae2230` added the game-input editing path and contextual dismantle fallback.
The bounded 512-entry scan and its progress-only watchdog were uncommitted
changes already present when this investigation started. HEAD still scanned
all entries synchronously. No live known-good baseline was supplied, so no
single commit is claimed as the proven origin of every reported symptom.

## Changes

- Read-only prop scans copy component data through the existing SEH guard,
  avoiding redundant VirtualQuery calls for each component. Address-only and
  write callers retain page validation.
- Use a steady-clock 2ms slice and an 8192-entry cap instead of 512; retain
  complete-result publication, epoch checks and a stall watchdog. Add a 120s
  absolute ceiling so tiny progress cannot keep a capture alive for hours.
- GetTransform prioritizes the known pointer after checking live table
  membership. It still verifies identity/generation and supports rediscovery.
- Known-handle undo no longer runs redundant region-wide prop queries. An
  unavailable transform API does not count as a confirmed removal.
- Reset preserves undo history; new capture cancels prior continuations;
  immediate prop failures return the save state to retryable readiness.
- Read library thumbnails on a bounded background worker, off the UI thread.

## Validation and limits

`cargo test -p shroudforge-api --test world_editor_building`: three tests pass,
one fresh-assets test skipped because no export was provided. Fixtures cover
save errors, cancelled capture continuations, retained undo, and handle-only
undo observation. Native prop-query fixture: 32 checks pass, including epoch
change, deadline, result caching, table membership and reused generation.
UI cargo check and Release executable/native DLL builds pass.

The corrected build has not been injected into the running game. Live capture,
paste and undo timing with this build still require a restart and a repeat of
the selection. No guarantee of save persistence or all multiplayer undo paths
is inferred from synthetic tests.
