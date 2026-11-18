# World Editor mod

This package restores the World Editor from the 0.9 reference with its existing
settings, SFBP V7 files, editor actions, library, rotation, paste/undo journal,
and Steam P2P protocol. The Lua code uses the public `runtime.world` and asset
APIs. It contains no game-code patches, fixed game RVAs, or function-hash lookups.

**Enshrouded 1.0 capture, placement and undo have been verified live.** The
session-bound voxel reader resolves `ecs::VoxelGrid` and `VoxelWorld` by type
name. Native writers and prop operations resolve the admitted original call
paths from registered names and retain the original ownership and Scene gates.
Local and dedicated-server tests include terrain, original-size props, rotation
and verified undo. Online capture captured 648 cells and two props, saved the V7 file
on the client, and the same blueprint was placed and undone on the server.
Existing Scene-owner resolution remains profile-backed. Save/reload persistence,
arbitrary prop scaling and a second connected game client are separate checks.
See the [dated live evidence](../../docs/evidence/runtime/2026-10-21-world-editor-native-operations.de.md).

The following describes the preserved editor workflow and required runtime
contract. See the current
[runtime guide](../../docs/runtime/native-runtime-and-game-profiles.en.md) and
[multiplayer guide](../../docs/modding/multiplayer-and-target-processes.de.md).

Read-only inspection of the installed 1.0 build `1269579` through the KFC4
parser found all ItemInfo field paths used by this mod. The preserved recipe
filter accepts 1,774 distinct placement recipes from 4,262 ItemInfo resources.
This verifies the file-resource contract, not loaded-world access or placement.

The client automatically uses the native direct path in a validated local
world and Steam P2P in a validated read-only server world. While the native
world context is unknown, Paste at cursor and Undo last paste pause without sending a local write or a
P2P request. Install and enable World Editor on both the client and the
Dedicated Server. The server runs the same mod headlessly. it has no UI or
keyboard hook. The client sends the existing SFBP V7 text through
`runtime.network.send_mod`, and the server parses it and invokes the same native
paste, snapshot, verification, and undo journal used by direct edits.

The loader's general **Network** module establishes the Steam session without
World Editor specific setup. It accepts incoming Steam session requests through
the game's callback pump and, on Dedicated Servers, sends a health probe every
five seconds to currently connected Enshrouded players. A client learns the
remote server SteamID64 from that probe. the remote-server fallback and
optional authorized-client list live in Settings > Modules > Network. An empty
authorized-client list uses the server's authenticated connected-player list,
read from `logs/enshrouded_server.log`. The World Editor uses this shared
service and keeps only its blueprint transfer, validation, authorization, and
undo protocol.
The P2P transfer limit is 32 MiB per
blueprint. Local blueprint files keep their existing 256 MiB limit. Online capture
sends the two marked world positions to the server. The server uses the same
native capture action and returns the complete V7 file in ordered chunks. The
client checks the sender, request ID, byte count and checksum, validates the
blueprint and saves it under the next available local `capture-N` name. Partial
or stale responses never become files. Reset / release cancels an online capture without
abandoning a placement journal. Paste at cursor sends V7 content to the server and Undo last paste sends
the server-issued undo token, so process-local handles never cross the network.

The server serializes complete capture, paste, and undo requests in a FIFO queue. Requests
are identified by authenticated peer plus request ID. Concurrent uploads have
separate buffers. Up to 32 requests can wait, with one waiting request per peer,
32 MiB of queued blueprint content and a separate 32 MiB upload budget. Queued
requests expire after 45 accumulated callback seconds and recheck authorization
and world session before dispatch. Queue progress is reported every two such
seconds. Repeated active requests do not dispatch again, and the last 128
terminal receipts are retained. Abandon requests release their own transaction
after active work completes and can cancel their own queued request.

Each client retains its own latest in-memory undo journal, with at most 32
clients retaining journals in one world session. A confirmed placement allows
another paste, including the same active blueprint. Undo last paste then restores only that
client's latest placement, as on the local editor path. A partial placement
still blocks that client's next paste until recovery or explicit abandonment.
Another client's journal is never overwritten, undone or released. The actual
undo checks still refuse to overwrite terrain or props changed since placement.
Restarting the server discards its journals. A lost paste reply can be recovered
through the same request receipt or the retained recovery token.

The P2P transport retains SFBP validation, authorization, and duplicate-request
guards from the reference implementation. **Simultaneous clients are covered by
automated protocol and journal tests. A second live game client and save/reload
persistence remain unverified.** A confirmed native readback
means the server runtime observed its world write. another client seeing the
change and the world retaining it after rejoin are separate checks. See the
[dated multiplayer investigation](../../docs/evidence/runtime/2026-10-09-world-editor-multiplayer.de.md)
for historical evidence only, not current 1.0 support.

This independent Lua mod implements its editor, blueprint format, selection, rotation, preview plan, paste, and undo behavior itself. It uses ShroudForge's public Lua APIs only for game/runtime access: `runtime.world.cursor.get`, native prop recipe registration and bounds queries, voxel operations, entity operations, asset reads, settings, logging, and export storage. Native engine access stays behind those APIs. the mod has no private loader hook or built-in editor implementation.

World Editor shortcuts are declared once as `control: "keybind"` settings in
`extended.mod.json`. Edit them under **Keyboard shortcuts** in the mod settings
or in **Compatibility → Hotkeys**. Changes use the existing live settings flow.
Bindings store one string with an SDL keyboard chord, a mouse chord, or both,
such as `SDL:64|Mouse:4`, with optional modifiers on each chord. The editor shows
separate keyboard and mouse columns. SDL supplies the displayed names. The
on-screen instructions and shortcut strip use the current bindings and update
when those settings or the keyboard layout change. A separate hidden ShroudForge
process observes in-game input and forwards its states to the Lua runtime.
An empty binding disables that shortcut. The editor keeps its existing 100 ms polling interval,
so a key must be held until an update observes it. Raw input is not consumed.

The actions rotate the active blueprint by 90° around its stored up axis, undo,
mark selection corners A/B, reset or release an unfinished operation, paste at
the live cursor, and capture/save/select a blueprint. After a server placement
or undo stops reporting progress for 10 seconds, its progress state turns red
and the editor offers Reset / release. Paste remains locked until the server
confirms release. The same actions are available as buttons in the mod settings.

The dashed **New blueprint** tile stays at the far left. Marking the first
corner starts a new capture and disarms the prior blueprint. Capture and save
selects the newly saved blueprint. Selecting any saved blueprint makes it ready
for paste immediately, without resetting the editor. The active card has a
persistent gold border and an **ACTIVE** badge, while a card being loaded has a
separate blue **LOADING** state. The library opens as a single row. The chevron
expands it downward, showing up to six rows before vertical scrolling reveals
additional rows. Selecting an item collapses the grid. The most recently
selected or created blueprint appears first after the permanent New blueprint
tile. Double-click or use the card’s **Edit** button to manage it. The manager
can browse a screenshot folder, select a screenshot, and apply it as the
blueprint cover. The screenshot folder is remembered. Common Enshrouded
screenshot folders are detected on first use. The **Settings** button opens the
Modloader UI at the World Editor module settings. The configured World Editor
window shortcut opens or hides the single editor window. The red **×** button
also hides it. Modloader and Debug Console use their own configurable shortcuts.

When the game starts, ShroudForge launches one World Editor window host using the same Tao/Wry window approach as the Modloader UI and Debug Console. The editor window is centered lower in the game HUD. Its instruction line is at the top and the blueprint cards are arranged underneath. The World Editor window shortcut only shows or hides this whole window. Other configured shortcuts perform their editor actions without changing window visibility. A single click loads/selects a blueprint and a double-click opens the blueprint manager. The manager discovers common Enshrouded screenshot folders, lets the user browse another folder, previews supported images, and copies the chosen image into the blueprint library as `.png` plus a thumbnail. It never modifies the original screenshot. New or copied `.sfbp` files placed in `exports/world-editor/blueprints` are discovered automatically. To start a capture, select the building hammer, choose a single voxel, aim at the first corner and use **Mark next selection corner**. aim at the opposite corner and use **Mark next selection corner** again, then use **Capture and save blueprint**. The mod does not attempt to inspect or select the hammer or voxel mode automatically.

The persistent `.sfbp` format writes and accepts `SHROUDFORGE_WORLD_BLUEPRINT_V7`, with either a voxel channel plus props or props alone. Each voxel cell stores its value and ShroudEdit-compatible coverage (`Unknown`, `Empty`, or `Occupied`). Captures classify successfully read non-zero cells as occupied and zero cells as empty, preserving the reference format's coverage convention. On paste, unknown cells preserve the target, empty cells clear it in replace mode, and additive mode skips empty cells. The file also stores the local selection extent and voxel-channel offset from the selection anchor, along with item IDs, native entity-template UUIDs, prop transforms, and the blueprint up axis. On paste, the cursor world position is the anchor, as in ShroudEdit. Voxel pastes are refused when the resulting channel origin is not aligned to the active grid. Props-only blueprints carry no voxel channel. Spawn requests the captured native template UUID and ItemInfo tracking recipe through the public runtime API and requires verified native support for the active build. A confirmed spawn must return the actual opaque live entity handle. Transform lookup, target replacement, and undo use that handle through native APIs. Blueprints store no build-specific runtime entity address.

The Lua voxel contract limits each read or write to 65,536 cells and requests the `voxel` channel's grid origin and cell size from the runtime. The session-bound reader resolves named services and uses the original registered read interface. It uses the original 0.5 m grid, validates tile tasks and publishes only a complete result. Fixture tests cover complete reads, pending reads and session changes. Capture and terrain writes have also been verified live on local and dedicated-server worlds. Prop capture registers `ItemInfo` placement AABBs through the Lua API, then the native query applies rotated, scaled recipe bounds before returning matching handles and transforms. Original-size props keep the original spawn scale without requiring another transform write. Other scales still require an admitted guarded scale writer. The mod does not provide a second component writer.

A failed transform read alone never confirms removal. Only `nil, "entity-absent"` from the session-bound original membership check means that the issued handle's identity is absent. Undo and replacement retain their journals when a snapshot is unreadable, a session changes, or a write outcome is uncertain. The original prop is recorded before a removal attempt, so an unavailable readback cannot lose its recovery entry or justify spawning a duplicate.

Undo reads the target voxels before restoring the saved snapshot and pauses if those cells changed after paste. It checks each pasted prop's live handle and transform before removing it. On a paste failure, the mod immediately attempts to restore the pre-paste voxel and prop snapshot, following ShroudEdit's rollback path. If a native operation cannot be verified, it retains recovery state for Undo last paste retry. For a dedicated-server operation with no progress for 10 seconds, the client shows that the server may be stuck and offers Reset / release to abandon the operation. Reset / release sends an authenticated, repeatable release request; the client keeps Paste at cursor locked until the server confirms it has discarded that operation's undo journal and released its lock. Press Reset / release again to resend if confirmation is lost. Abandoning may leave partial terrain or props in the world, and the old operation can no longer be undone. After confirmation, another World Editor operation can start. If the server is unreachable, Paste at cursor remains paused until it receives and confirms the release request. Paste at cursor uses the live cursor position for its target. In the Modloader UI, open **World Editor**:

- Set **Blueprint up axis** to X, Y, or Z before capturing. The axis is stored in the blueprint. changing this setting later does not change an already captured or loaded blueprint.
- Set **Initial paste rotation** to 0, 1, 2, or 3 quarter turns (0°, 90°, 180°, 270°), then use **Rotate blueprint 90°** in game to advance the active blueprint by 90° each time. The Modloader's **Rotate blueprint 90°** button does the same. Rotation cycles through 0°, 90°, 180°, and 270° before returning to 0°. it applies to preview and paste.
- Click **Preview placement plan** to resolve the live cursor target and report the rotated dimensions and counts in the ShroudForge log. This preview is non-destructive, but it is a plan preview only: it does not draw a 3D ghost or show the schematic in the game world.
- Use **Paste active blueprint** or Paste at cursor only after checking the plan. Those actions write to the world.

Mark selection corners A and B, then use **Capture props only from A/B selection** to create a blueprint without a voxel channel. Use **Capture voxels and props** or Capture and save blueprint for a combined capture. Props-only blueprints can be pasted at the live cursor, a marked target, or the **Props-only target world X/Y/Z** settings without reading voxel grid metadata. Choose whether to replace or add voxel cells and whether to keep or replace intersecting target props. Replacing target props is refused when their recipe, native template identity, or transform cannot be safely verified. Capture and save blueprint names new persistent combined captures `capture-1`, `capture-2`, and so on, without replacing an existing export.

Set **Maximum props per blueprint** in the mod settings to control the per-blueprint prop count (default 60,000. supported range 1–1,000,000). Capture, save, load, and paste all enforce the current value. captures above it are refused instead of silently truncated.

Prop capture scans incrementally: each native scan slice checks at most 8,192 entity pointers and yields after approximately 2 ms of scan time. Initialization, final result validation, and individual memory reads are outside a hard real-time guarantee. Lua continues pending scans on subsequent updates. Incomplete results never become a saved blueprint. A changed ECS epoch, unavailable world, 30 seconds without native scan progress, or the 120-second overall scan deadline aborts capture. Reset / release cancels a local pending editor callback. For a remote server edit, Reset / release requests an authenticated server release and waits for confirmation. Recipe loading and voxel/file operations have separate execution paths. Capture stage messages in the log distinguish recipe resolution, pending enumeration, and completion.

The standalone mod implements its editor and V7 blueprint format itself. Direct world operations use the registered native Lua APIs. In multiplayer, the server executes those same APIs. the client never writes a replicated read-only grid or sends runtime entity handles. Runtime behavior that depends on the live game, including collision, replication and save persistence, requires in-game verification.

The World Editor header shows the active operation and its current phase for capture/save, load, library refresh/rename/duplicate/delete, preview, placement, rollback, and undo. It reports a step count only when the operation has a known number of coarse phases. otherwise the bar is indeterminate and shows elapsed time. Progress and visible UI messages are written through the shared logger to the role-appropriate ShroudForge log under the `world-editor` source. there is no separate UI log. INFO records meaningful phase changes and confirmed results, WARN records blocked or uncertain work, ERROR records failed writes or verification, DEBUG records inputs, target details, readiness changes, and window visibility, and TRACE records deduplicated full UI snapshots. See the current [logging API](https://shroudforge.net/tools/modding/api-reference/) for new messages. In multiplayer the server logs its own native phases and sends phase updates over the existing P2P request so the client can show them and record the received server status in its own World Editor log. Live readback confirms the in-world result. it does not confirm save persistence.
