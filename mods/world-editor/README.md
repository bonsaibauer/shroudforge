# World Editor mod

The client automatically uses the native direct path in a validated local
world and Steam P2P in a validated read-only server world. While the native
world context is unknown, F7 and F4 pause without sending a local write or a
P2P request. Install and enable World Editor on both the client and the
Dedicated Server. The server runs the same mod headlessly; it has no UI or
keyboard hook. The client sends the existing SFBP V7 text through
`runtime.network.send_mod`, and the server parses it and invokes the same native
paste, snapshot, verification, and undo journal used by direct edits.

For a client and Dedicated Server running in the same Windows session, the
client discovers the server's current SteamID64 automatically. Enshrouded can
assign a new ID after a server restart, so the client reads the live ID from the
running server instead of relying on a stale saved value. For remote servers,
the client's **Dedicated server SteamID64 fallback** field remains available.
The server automatically accepts P2P sessions only for players Enshrouded has
authenticated and still lists as connected. The native server runtime derives
that live set from the Dedicated Server's active `logs/enshrouded_server.log`
join, authentication, and removal events; the World Editor receives only the
resulting SteamID64 list and calls `runtime.network.accept` before its P2P
receiver polls. **Authorized client SteamID64 list** remains an optional strict
override for administrators. This ties P2P access to the server's logged Steam
authentication lifecycle; it does not infer authorization from a client hello.
The P2P transfer limit is 32 MiB per
blueprint; local blueprint files keep their existing 256 MiB limit. Capturing
and saving remain on the client; F7 sends the same V7 blueprint body to the
server. F4 sends the server-issued undo token back, so process-local entity
handles never cross the network. Switching between local and server worlds
changes the route automatically. The dedicated server keeps one in-memory undo
journal; restarting it discards that journal. If the client loses a paste
reply while the server retains an unresolved journal, a later paste refusal
returns that peer's existing undo token so F4 can resume recovery. A stale
client undo token cannot permanently block F7: the client may submit the next
paste, and the server accepts it only after its own prior undo journal is
complete. If an undo is still active, the server keeps rejecting edits until
it reports completion.

The P2P transport, SFBP validation, authorization, and duplicate-request guards
are covered by isolated tests. **Live dedicated-server replication and save
persistence still require an in-game canary test.** A confirmed native readback
means the server runtime observed its world write; another client seeing the
change and the world retaining it after rejoin are separate checks. See the
[per-mod execution audit](../../docs/sf/mod-multiplayer.md) for the broader
runtime evidence.

This independent Lua mod implements its editor, blueprint format, selection, rotation, preview plan, paste, and undo behavior itself. It uses ShroudForge's public Lua APIs only for game/runtime access: `runtime.world.cursor.get`, native prop recipe registration and bounds queries, voxel operations, entity operations, asset reads, settings, logging, and export storage. Native engine access stays behind those APIs; the mod has no private loader hook or built-in editor implementation.

While the game window is focused, the World Editor polls **F3–F8**: F3 rotates the active blueprint by 90° around its stored up axis, F4 undo, F5 mark selection corner A/B, F6 reset editor state, F7 paste at the live cursor, and F8 capture, save, and select a blueprint. **F1 remains available to Enshrouded.** The dashed **New blueprint** tile stays at the far left. Starting a new capture with F5 automatically makes that tile active and disarms the prior blueprint; after F8, the new saved blueprint becomes active. Selecting any saved blueprint makes it ready for F7 immediately; F6 is not required to switch blueprints. The active card has a persistent gold border and an **ACTIVE** badge, while a card being loaded has a separate blue **LOADING** state. The library opens as a single row; the chevron expands it downward, showing up to six rows before vertical scrolling reveals additional rows. Selecting an item collapses the grid. The most recently selected or created blueprint appears first after the permanent New blueprint tile. Double-click or use the card’s **Edit** button to manage it. The manager can browse a screenshot folder, select a screenshot, and apply it as the blueprint cover. The screenshot folder is remembered; common Enshrouded screenshot folders are detected on first use. The **Settings** button opens the Modloader UI at the World Editor module settings. **F2 opens or hides the single World Editor window**; the red **×** button also hides it. **F9 remains the Modloader UI shortcut; F10 remains the Debug Console shortcut.** The same editor actions are available as buttons in the mod settings.

When the game starts, ShroudForge launches one World Editor window host using the same Tao/Wry window approach as the Modloader UI and Debug Console. The fixed-size window is centered lower in the game HUD. Its instruction line is at the top and the blueprint cards are arranged underneath. F2 only shows or hides this whole window; other function keys perform their editor actions without changing window visibility. A single click loads/selects a blueprint and a double-click opens the blueprint manager. The manager discovers common Enshrouded screenshot folders, lets the user browse another folder, previews supported images, and copies the chosen image into the blueprint library as `.png` plus a thumbnail. It never modifies the original screenshot. New or copied `.sfbp` files placed in `exports/world-editor/blueprints` are discovered automatically. To start a capture, select the building hammer, choose a single voxel, aim at the first corner and press F5; aim at the opposite corner and press F5 again, then press F8. The mod does not attempt to inspect or select the hammer or voxel mode automatically.

The persistent `.sfbp` format writes and accepts `SHROUDFORGE_WORLD_BLUEPRINT_V7`, with either a voxel channel plus props or props alone. Each voxel cell stores its value and ShroudEdit-compatible coverage (`Unknown`, `Empty`, or `Occupied`); captures classify successfully read non-zero cells as occupied and zero cells as empty, matching the active Shroudtopia read API. On paste, unknown cells preserve the target, empty cells clear it in replace mode, and additive mode skips empty cells. The file also stores the local selection extent and voxel-channel offset from the selection anchor, along with item IDs, native entity-template UUIDs, prop transforms, and the blueprint up axis. On paste, the cursor world position is the anchor, as in ShroudEdit. Voxel pastes are refused when the resulting channel origin is not aligned to the active grid; props-only pastes do not require voxel APIs. Spawn uses the captured native template UUID and ItemInfo tracking recipe through KFC Runtime's profile-backed entity spawn operation. Spawn returns the actual opaque live entity handle; transform lookup, target replacement, and undo use that handle through native APIs. Blueprints store no build-specific runtime entity address.

Voxel reads and writes are bounded to 65,536 cells, matching the active Shroudtopia backend; writes are aligned to 8-cell chunk boundaries, dispatched on the game thread, and checked with native readback. The backend currently exposes one grid channel, `voxel`, with a 0.5 m cell size. A successful readback confirms voxel data only; collision and save persistence still require separate verification. Prop capture registers `ItemInfo` placement AABBs through the Lua API, then the dedicated native world query scans the live entity snapshot and applies rotated, scaled recipe bounds before returning matching prop handles and transforms. Transform lookup, scale updates, spawn, target replacement, and removal use registered native world APIs. The scale update writes and verifies only the scale fields of the live `CurrentTransform` component on the game thread; the Lua mod does not use generic ECS calls.

Undo reads the target voxels before restoring the saved snapshot and pauses if those cells changed after paste. It checks each pasted prop's live handle and transform before removing it. On a paste failure, the mod immediately attempts to restore the pre-paste voxel and prop snapshot, following ShroudEdit's rollback path; if a native operation cannot be verified, it retains recovery state for F4 retry. An incomplete recovery does not block capture, blueprint selection, or F6 editor reset; those tools preserve the recovery journal and F4 remains available. F7 stays paused until recovery completes, so another placement cannot compound a partially applied change. F7 uses the live cursor position for its target. In the Modloader UI (F9), open **World Editor**:

- Set **Blueprint up axis** to X, Y, or Z before capturing. The axis is stored in the blueprint; changing this setting later does not change an already captured or loaded blueprint.
- Set **Initial paste rotation** to 0, 1, 2, or 3 quarter turns (0°, 90°, 180°, 270°), then press **F3** in game to advance the active blueprint by 90° each time. The Modloader's **Rotate blueprint (F3)** button does the same. Rotation cycles through 0°, 90°, 180°, and 270° before returning to 0°; it applies to preview and paste.
- Click **Preview placement plan** to resolve the live cursor target and report the rotated dimensions and counts in the ShroudForge log. This preview is non-destructive, but it is a plan preview only: it does not draw a 3D ghost or show the schematic in the game world.
- Use **Paste active blueprint** or F7 only after checking the plan. Those actions write to the world.

Mark selection corners A and B, then use **Capture props only from A/B selection** to create a blueprint without a voxel channel. Use **Capture voxels and props** or F8 for a combined capture. Props-only blueprints can be pasted at the live cursor, a marked target, or the **Props-only target world X/Y/Z** settings without reading voxel grid metadata. Choose whether to replace or add voxel cells and whether to keep or replace intersecting target props. Replacing target props is refused when their recipe, native template identity, or transform cannot be safely verified. F8 names new persistent combined captures `capture-1`, `capture-2`, and so on, without replacing an existing export.

Set **Maximum props per blueprint** in the mod settings to control the per-blueprint prop count (default 60,000; supported range 1–1,000,000). Capture, save, load, and paste all enforce the current value; captures above it are refused instead of silently truncated.

Prop capture scans incrementally: each native scan slice checks at most 512 entity pointers and yields after approximately 2 ms of scan time. Initialization and individual memory reads are outside a hard real-time guarantee. Lua continues pending scans on subsequent updates; incomplete results never become a saved blueprint. A changed ECS epoch, unavailable world, or 30 seconds without native scan progress aborts the capture; a scan that continues advancing is not cancelled merely because its total duration exceeds 30 seconds. F6 discards the pending editor callback. This bounds enumeration work; recipe loading and voxel/file operations have separate execution paths. Capture stage messages in the log distinguish recipe resolution, pending enumeration, and completion.

The standalone mod implements its editor and V7 blueprint format itself. Direct world operations use the registered native Lua APIs. In multiplayer, the server executes those same APIs; the client never writes a replicated read-only grid or sends runtime entity handles. Runtime behavior that depends on the live game, including collision, replication and save persistence, requires in-game verification.

The World Editor header shows the active operation and its current phase for capture/save, load, library refresh/rename/duplicate/delete, preview, placement, rollback, and undo. It reports a step count only when the operation has a known number of coarse phases; otherwise the bar is indeterminate and shows elapsed time. Progress is updated alongside the existing `shroudforge.log` output under the `world-editor` source. INFO records meaningful phase changes and results, WARN records pauses or recoverable problems, ERROR records failed writes or incomplete recovery, and DEBUG keeps repetitive readiness details out of the normal log view. In multiplayer the server logs its own native phases and sends phase updates over the existing P2P request so the client can show them and record the received server status in its own World Editor log. Live readback confirms the in-world result; it does not confirm save persistence.
