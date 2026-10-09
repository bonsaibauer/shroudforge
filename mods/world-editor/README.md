# World Editor mod

**Multiplayer input mode:** choose **World edit execution → Game building input (multiplayer)**
in the mod settings. This new path queues ordinary player building input through
`runtime.world.building.submit/status/cancel`, waits for item selection and then
observes each requested change. It does not require a second editor instance on
the dedicated server. The default **Direct world API (local/host)** keeps the
existing direct operations described below. Running both EXEs is not enough:
the client must join the intended server.

The input adapter and job logic are built and checked against original layouts,
code guards and isolated tests. **End-to-end multiplayer acceptance, replication
and persistence have not yet been verified with the new DLL.** See the
[per-mod execution audit](../../docs/sf/mod-multiplayer.md) for exact scope and
the distinction between active patches and gameplay effects.

The game-input path keeps the building hammer equipped, resolves exact one-cell
building material recipes from current KFC assets, and preserves normal game
permissions and resource checks. Terrain without an exact recipe is refused
before submission. Jobs allow at most 4096 actions. Removing a prop additionally
waits for the game's cursor to target that prop; changing the transform alone
does not prove the correct interaction target. Once targeted, the editor emits
the reflected `ContextualAction` and `ContextualAction_Hold` inputs for 1.2
seconds, matching the held interaction used to dismantle workbenches and other
props. Voxel removal continues to use `SecondaryBuildingAction`. The editor
displays the target coordinates. F4 stops a running queue, then undoes observed
actions through inverse input. A prop already dismantled manually counts as an
already completed inverse action. Late effects are observed without retrying the original action;
an unconfirmed outcome pauses further writes. Closing the mod cancels pending
input but cannot undo an already dispatched action. Journals are session-local.

In direct mode, F4 first tries the checked native destroy operation. If the
current client never enters its profiled destroy hook, the editor keeps the
exact handle and transform guard and falls back to the same held contextual
input. It asks the player to aim at that prop, observes its disappearance, and
then continues the remaining prop and voxel rollback automatically.

This independent Lua mod implements its editor, blueprint format, selection, rotation, preview plan, paste, and undo behavior itself. It uses ShroudForge's public Lua APIs only for game/runtime access: `runtime.world.cursor.get`, native prop recipe registration and bounds queries, voxel operations, entity operations, asset reads, settings, logging, and export storage. Native engine access stays behind those APIs; the mod has no private loader hook or built-in editor implementation.

While the game window is focused, the World Editor polls **F3–F8**: F3 rotates the active blueprint by 90° around its stored up axis, F4 undo, F5 mark selection corner A/B, F6 reset editor state, F7 paste at the live cursor, and F8 capture, save, and select a blueprint. **F1 remains available to Enshrouded.** The dashed **New blueprint** tile stays at the far left. Starting a new capture with F5 automatically makes that tile active and disarms the prior blueprint; after F8, the new saved blueprint becomes active. Selecting any saved blueprint makes it ready for F7 immediately; F6 is not required to switch blueprints. The active card has a persistent gold border and an **ACTIVE** badge, while a card being loaded has a separate blue **LOADING** state. The library opens as a single row; the chevron expands it downward, showing up to six rows before vertical scrolling reveals additional rows. Selecting an item collapses the grid. The most recently selected or created blueprint appears first after the permanent New blueprint tile. Double-click or use the card’s **Edit** button to manage it. The manager can browse a screenshot folder, select a screenshot, and apply it as the blueprint cover. The screenshot folder is remembered; common Enshrouded screenshot folders are detected on first use. The **Settings** button opens the Modloader UI at the World Editor module settings. **F2 opens or hides the single World Editor window**; the red **×** button also hides it. **F9 remains the Modloader UI shortcut; F10 remains the Debug Console shortcut.** The same editor actions are available as buttons in the mod settings.

When the game starts, ShroudForge launches one World Editor window host using the same Tao/Wry window approach as the Modloader UI and Debug Console. The fixed-size window is centered lower in the game HUD. Its instruction line is at the top and the blueprint cards are arranged underneath. F2 only shows or hides this whole window; other function keys perform their editor actions without changing window visibility. A single click loads/selects a blueprint and a double-click opens the blueprint manager. The manager discovers common Enshrouded screenshot folders, lets the user browse another folder, previews supported images, and copies the chosen image into the blueprint library as `.png` plus a thumbnail. It never modifies the original screenshot. New or copied `.sfbp` files placed in `exports/world-editor/blueprints` are discovered automatically. To start a capture, select the building hammer, choose a single voxel, aim at the first corner and press F5; aim at the opposite corner and press F5 again, then press F8. The mod does not attempt to inspect or select the hammer or voxel mode automatically.

The persistent `.sfbp` format writes and accepts `SHROUDFORGE_WORLD_BLUEPRINT_V7`, with either a voxel channel plus props or props alone. Each voxel cell stores its value and ShroudEdit-compatible coverage (`Unknown`, `Empty`, or `Occupied`); captures classify successfully read non-zero cells as occupied and zero cells as empty, matching the active Shroudtopia read API. On paste, unknown cells preserve the target, empty cells clear it in replace mode, and additive mode skips empty cells. The file also stores the local selection extent and voxel-channel offset from the selection anchor, along with item IDs, native entity-template UUIDs, prop transforms, and the blueprint up axis. On paste, the cursor world position is the anchor, as in ShroudEdit. Voxel pastes are refused when the resulting channel origin is not aligned to the active grid; props-only pastes do not require voxel APIs. Spawn uses the captured native template UUID and ItemInfo tracking recipe through KFC Runtime's profile-backed entity spawn operation. Spawn returns the actual opaque live entity handle; transform lookup, target replacement, and undo use that handle through native APIs. Blueprints store no build-specific runtime entity address.

Voxel reads and writes are bounded to 65,536 cells, matching the active Shroudtopia backend; writes are aligned to 8-cell chunk boundaries, dispatched on the game thread, and checked with native readback. The backend currently exposes one grid channel, `voxel`, with a 0.5 m cell size. A successful readback confirms voxel data only; collision and save persistence still require separate verification. Prop capture registers `ItemInfo` placement AABBs through the Lua API, then the dedicated native world query scans the live entity snapshot and applies rotated, scaled recipe bounds before returning matching prop handles and transforms. Transform lookup, scale updates, spawn, target replacement, and removal use registered native world APIs. The scale update writes and verifies only the scale fields of the live `CurrentTransform` component on the game thread; the Lua mod does not use generic ECS calls.

Undo reads the target voxels before restoring the saved snapshot and pauses if those cells changed after paste. It checks each pasted prop's live handle and transform before removing it. On a paste failure, the mod immediately attempts to restore the pre-paste voxel and prop snapshot, following ShroudEdit's rollback path; if a native operation cannot be verified, it retains recovery state for F4 retry. F6 will not discard incomplete recovery state. F7 uses the live cursor position for its target. In the Modloader UI (F9), open **World Editor**:

- Set **Blueprint up axis** to X, Y, or Z before capturing. The axis is stored in the blueprint; changing this setting later does not change an already captured or loaded blueprint.
- Set **Initial paste rotation** to 0, 1, 2, or 3 quarter turns (0°, 90°, 180°, 270°), then press **F3** in game to advance the active blueprint by 90° each time. The Modloader's **Rotate blueprint (F3)** button does the same. Rotation cycles through 0°, 90°, 180°, and 270° before returning to 0°; it applies to preview and paste.
- Click **Preview placement plan** to resolve the live cursor target and report the rotated dimensions and counts in the ShroudForge log. This preview is non-destructive, but it is a plan preview only: it does not draw a 3D ghost or show the schematic in the game world.
- Use **Paste active blueprint** or F7 only after checking the plan. Those actions write to the world.

Mark selection corners A and B, then use **Capture props only from A/B selection** to create a blueprint without a voxel channel. Use **Capture voxels and props** or F8 for a combined capture. Props-only blueprints can be pasted at the live cursor, a marked target, or the **Props-only target world X/Y/Z** settings without reading voxel grid metadata. Choose whether to replace or add voxel cells and whether to keep or replace intersecting target props. Replacing target props is refused when their recipe, native template identity, or transform cannot be safely verified. F8 names new persistent combined captures `capture-1`, `capture-2`, and so on, without replacing an existing export.

Set **Maximum props per blueprint** in the mod settings to control the per-blueprint prop count (default 60,000; supported range 1–1,000,000). Capture, save, load, and paste all enforce the current value; captures above it are refused instead of silently truncated.

The standalone mod implements its editor and V7 blueprint format itself. Direct world operations use the registered native Lua APIs. The game-input queue additionally reads ClientPlayerInput, NetworkCursor and SlotSelection through the shared ECS API to identify the local player and observe selection; it does not write arbitrary ECS fields. Runtime behavior that depends on the live game, including recipe acceptance, collision, replication and save persistence, requires in-game verification.
