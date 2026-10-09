# Client world context integration (2026-10-09)

Baseline: `f38c172`. Current revision: `world-context-session-20261009-a2`.

## Scope and remaining prerequisite

This revision adds a client-cursor read adapter, context-source evidence and
isolation across ECS session generations. The existing direct singleton/actor
resolution order and native removal behavior remain unchanged. The existing
cursor callback receives RBP as a third argument; register preservation and
stack allocation are unchanged. The client profile gates the extra frame
layout on exact EXE identity; server and unknown-build profiles do not enable it.

Three cases must remain separate:

| Case | Role | World Editor backend |
| --- | --- | --- |
| Dedicated process | `enshrouded_server.exe` | Client editor remains disabled |
| Client in local/host world | `enshrouded.exe` | Existing direct backend |
| Client joining a dedicated world | `enshrouded.exe` | Existing game-input backend; requires readable replicated world for capture/effect verification |

`runtime.is_server` identifies the process, not a client's connection mode.
`runtime.world.session_id()` is an ECS lifetime generation, not a persistent
world identity. The persisted `executionMode=direct` now prefers direct access
when available and selects game input if only client-grid reads are available.
This is a capability-based backend choice, not an automatically detected network
mode. The native diagnostic reports
`clientSessionMode=unknown` rather than guessing from missing pointers or
whether a server process is running.

## Evidence from the installed client

Client PID 22192, server PID 15940 were running. The engine log records local
`Host_Offline` followed by `Client_Online`. The client ECS manager and dispatcher
were ready, but `voxel_context=waiting`; the editor had `saveState=saving` and
an unreconciled input from a prior world. A successful early paste predates the
server start and is evidence for singleplayer only.

Read-only memory inspection of the exact client image found the configured
global at RVA `0x273f588` equal to zero in the remote session. Static inspection
of the EXE establishes:

* `world_prop_update` at `0x282496` and `world_actor_placement` at `0x282965`
  both belong to `player_building_place_prop`, starting at `0x282380`.
* The separate `client_cursor` starts at `0x249030`; its existing hook at
  `0x24aa1d` receives the execution view in R13. The existing callback already
  observes the local ECS manager through that view.
* The named `client_cursor` descriptor at `0x1d1d770` lists `VoxelWorld` among
  its resource dependencies. Instructions `0x24984d/0x249854` copy `[rbp+0x250]`
  into `[rbp+0x4e8]`; `0x249f59` loads it into RCX and calls `0x99e530`, which
  returns `[rcx+8]`. This value is passed to `0xe8b710`, then as R8 to the
  existing voxel reader `0xe819d0`, with mode 6. The verifier
  `verify-client-voxel-context.py` checks this chain in the actual EXE.
  Server executable addresses are not reused.

The profile label `runtime-pointer-validation-required` describes the validation
method, not a current pointer failure. `voxelContextActive`, the actual probe
reasons and live hook counts provide the current evidence.

## Changes

* Native diagnostics report process role, actual selected world source,
  singleton/context/store failure stage, last actor observation, hook counts,
  resets and the local execution root. Hook callbacks only record atomics;
  serialization runs in the diagnostic/status path.
* The cursor world is a separate, expiring, manager-bound **read-only** source.
  A successful read never pins it into the direct context. Direct writes/spawns
  continue to require the original context. Queued reads carry a generation
  and stop if the context changes before execution.
* A queued world operation requires a nonzero session, has a finite retry window
  (15 accumulated update seconds; the loader may skip callbacks),
  and cannot deliver its continuation after that session disappears or changes.
  Active incremental prop scans retain their existing progress watchdog.
* Before editor updates and UI actions, a new nonzero session retires old input
  work and archives its pending result and undo journal in memory. Zero alone
  does not discard undo. Old outcomes cannot reconcile against a new world.
* World changes cancel captures and clear old selections, targets, previews and
  prop caches. A loaded/captured blueprint stays available. Archives are for
  inspection during this mod lifetime; they are not cross-session replay data.
* Cursor snapshots carry a generation, preventing use after a context reset.
* Readiness now includes voxel-read availability. The helper state distinguishes
  process role, selected backend and session generation.

## Validation

Native context fixtures exercise the existing singleton and actor sources,
null singleton, bad actor frame, cache reset, manager switching and local cursor
priority. The obsolete R10 dispatcher-removal expectations were corrected to
the restored R9 behavior; the removed consumer was not reintroduced.

Lua regression fixtures cover successful capture/export, failed export,
incremental scans, finite waits, readiness appearing in the wrong session,
temporary session loss, session retirement and no replay/reconciliation of old
input in the new world. Building-input fixtures protect the existing ABI path.

The a1 live test confirmed thousands of cursor callbacks while both placement
hooks stayed at zero and the singleton stayed null. A failed F8 capture ended
in `saveState=error` with an explicit timeout instead of remaining `saving`.

A2 was installed with hashes verified. Client PID 19836 and server PID 6668
were responding. The engine log confirms `Client_Online`; native status reports
`source=client-cursor-read`, `voxel_context=ready`, with the singleton still null
and both prop hooks unused. F8 captured 576 cells (192 occupied), zero props,
and exported `capture-6.sfbp`; editor state reached `saveState=complete`, 4/4,
with `executionBackend=game-input`, session generation 4. This proves remote
client capture/export, not server placement or persistence.

The subsequent F7 attempts were rejected by the existing recipe preflight:
`Selection includes terrain or a cell without an exact one-cell building recipe`.
No input is sent when this branch returns. A fresh read of the installed EXE/KFC
through `inspect_building_inputs` found 66 exact single-cell recipes, including
material 192 in capture-6. All four Lua tests, including the fresh-resource test,
pass. The old combined error did not identify whether the rejected cell was
in the blueprint or destination, so it cannot establish which condition failed.
Inspection of saved captures shows that capture-3 and capture-5 contain terrain
density encodings (including 65281), while capture-6 contains only 0 and 192.
The existing game-input planner intentionally does not translate terrain into
single building blocks. Capture-6 is the appropriate isolated block test.

A Lua-only preflight diagnostic now returns the side, raw value and reason,
and logs coordinates, old/new values, recipe count and paste mode. It does not
change the acceptance rule or issue additional game inputs. Installed files
were backed up in `world-editor-preflight-20261009-160139`; the loader confirmed
the explicit editor-only reload as `reloaded`. The A2 native DLL is unchanged.
The supplemental `world-editor-preflight-patch.json` records these Lua hashes.

The next live F7 with capture-6 pinpointed the rejection at client elapsed
00:08:27.494: `side=Target cell=19457 xyz=7495,1685,2875 old=19457 new=192
recipes=66 mode=replace`. Thus the current rejection is the destination terrain
encoding, not a missing block recipe or world-context outage. The planner
rejects the entire batch before input because it cannot invert that terrain
replacement using its single-block recipes. Switching to `add` does not solve
this occupied-source-cell overlap: both modes validate the old cell before
replacing it with 192. A block-only test must place the occupied blueprint cells
clear of terrain; terrain-capable multiplayer placement remains unimplemented.
Both live processes remained responsive, editor session generation stayed 4.

Follow-up analysis of `selected building item was not observed` found two
separate game selections. The live cursor held item `1986896347`, resolved in
the installed KFC as `Blueprint_Voxel_Block_05m` (zero-based registry index
21), while the source block's material is `Block_T5_Wood_Pine` item
`2615018308` (registry index 88, voxel material 192). They are different
fields: `CreateBuildingItemAction` selects the voxel shape; `BuildingStockCycleAction`
sets the default material. The initial selection-index patch passed the material
item as the shape and failed; this latest catalog change instead selects the
generic 0.5 m shape and sends the material item's ID through the material field.
The stock C++ adapter already writes the provided material ID to the reflected
default/terrain material fields. The shape/material split is installed in the
client and its loader acknowledged `reloaded`; its backup is
`world-editor-shape-material-20261009-161745`. The live remote test must confirm
the engine and server accept both inputs before this is considered fixed.

Remaining live acceptance: successful remote placement/undo, A2 SP
capture/place/undo, world switching with pending capture/input, and server
persistence after rejoin. A native test fixture does not prove these outcomes.

## Diagnostic incident

Two CDB read commands encountered null-pointer expression errors before their
queued detach command. Although the debugger exited, each left one suspend
count on the 50 client threads. This was detected from stopped logs and thread
states; both counts were explicitly resumed. The client then responded and
logs advanced again. No EXE/world memory was written. Further evidence should
use `OpenProcess` with read/query access (the existing `discovery.live.Process`)
without suspending the game. This incident must not be mistaken for a failure
of the new DLL: no new DLL had been installed.

## Latest F7 result and repeated-query stall

On the next `capture-6` F7, shape selection passed and the job reached its
effect-observation phase, but the target cell stayed `0` instead of wood
material `192` at grid cell `(7475,1688,2875)` for the 15-second observation
window. The selected shape was `1986896347`, the wood material item was
`2615018308`, and the selection index was `21`. This proves the local input
adapter progressed beyond selection; it does not prove server acceptance or
identify a server rejection. The stock Dedicated Server log has no
action-specific acceptance/rejection record. The failure is retained as
uncertain and is not retried automatically.

Two read-only runtime snapshots ten minutes apart show the separate stall
mechanism while the unresolved action remained in recovery: general ECS
queries rose from 5,542 to 10,188; incomplete scans from 5,436 to 9,981; query
timeouts from 32 to 83. The active scan moved from 15,119 to 85,137 of 131,072
entities. In source, `game_building.tick()` re-ran the full-table local-player
query on every update, and `reconcile()` did the same forever after an
unconfirmed input. The current source change caches the generation-checked
player handle for a session, reuses it throughout its queued job, and reconciles
only the original command against the same session. No input is replayed.

The next placement diagnostic records the live cursor validity flags and
primary/secondary world positions beside the requested transform. This will
distinguish an invalid client cursor frame from a server-side non-acceptance;
the exact effect failure is not yet resolved. The live mod has not been
hot-reloaded with these source edits because doing so would discard its
in-memory uncertain-input guard. The client must be restarted before this
Lua revision can be loaded safely.

## Timeout confirmation after the latest F7

The user confirmed that the new timeout message appeared. The persisted editor state gives the exact reason: `voxel effect not observed yet (at=7475,1688,2875 expected=192 current=0 materialItemId=2615018308)`. Static native-code review confirms that `BuildingInput::Observe` setting status `3` means only that the input was dispatched; the code explicitly says it is not a world-effect acknowledgement. Therefore this message means the action was injected but the voxel read never observed the expected result. Current logs contain no server-side accept/reject reason.

The optimized Lua file with the player-handle cache and cursor trace is installed and hash-matched on disk, but its `reloadRequested` flag is false; it was deliberately not hot-reloaded while the prior in-memory input was uncertain. At inspection time the Dedicated Server (PID 6668) remained running and the client process was no longer present. A client restart is required to load the trace. On that run, the trace will show the pre-injection cursor flags and coordinates; a matching valid cursor frame narrows the next check to the server/game permission path, while a mismatch identifies the client-side cursor/transform injection.
