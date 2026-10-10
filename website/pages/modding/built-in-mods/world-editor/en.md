# World Editor

Copy, rotate, save, and paste parts of the world.

## Package details

- Mod ID: `world-editor`
- Target processes: client and Dedicated Server
- Runtime capability: yes
- Declared player settings: 37

## Settings

## In-game panel

Position is relative to the game window in logical pixels. X = −1 centers the panel. Y is measured down from the top. Changes apply while the game is running.

- [Blueprint panel width](#doc-builtin-world-editor-panelwidth)
- [Blueprint panel height](#doc-builtin-world-editor-panelheight)
- [Panel position X (−1 = centered)](#doc-builtin-world-editor-panelpositionx)
- [Panel position Y (from game top)](#doc-builtin-world-editor-panelpositiony)

## World Editor limits

- [Maximum props per blueprint](#doc-builtin-world-editor-maximumcopyableprops)

## Native entity operations

Spawn and placement use native game operations.

- [Template UUID high (hex)](#doc-builtin-world-editor-templateuuidhigh)
- [Template UUID low (hex)](#doc-builtin-world-editor-templateuuidlow)
- [Tracking ID](#doc-builtin-world-editor-trackingid)
- [Position X](#doc-builtin-world-editor-entityx)
- [Position Y](#doc-builtin-world-editor-entityy)
- [Position Z](#doc-builtin-world-editor-entityz)
- [Material feedback ID (placement only)](#doc-builtin-world-editor-feedbackid)
- [Bounds min X](#doc-builtin-world-editor-boundsminx)
- [Bounds min Y](#doc-builtin-world-editor-boundsminy)
- [Bounds min Z](#doc-builtin-world-editor-boundsminz)
- [Bounds max X](#doc-builtin-world-editor-boundsmaxx)
- [Bounds max Y](#doc-builtin-world-editor-boundsmaxy)
- [Bounds max Z](#doc-builtin-world-editor-boundsmaxz)

## World Editor

Capture voxel-and-prop or props-only blueprints, preview, paste, and undo through native world APIs.

- [Source cell X (manual fallback)](#doc-builtin-world-editor-sourcex)
- [Source cell Y (manual fallback)](#doc-builtin-world-editor-sourcey)
- [Source cell Z (manual fallback)](#doc-builtin-world-editor-sourcez)
- [Size X (manual fallback)](#doc-builtin-world-editor-sizex)
- [Size Y (manual fallback)](#doc-builtin-world-editor-sizey)
- [Size Z (manual fallback)](#doc-builtin-world-editor-sizez)
- [Persistent blueprint name](#doc-builtin-world-editor-blueprintname)
- [New name for rename or duplicate](#doc-builtin-world-editor-blueprintnewname)
- [Initial paste rotation (F3 cycles in game, 0–3)](#doc-builtin-world-editor-rotationquarterturns)
- [Blueprint up axis (stored on capture)](#doc-builtin-world-editor-rotationaxis)
- [Voxel mode](#doc-builtin-world-editor-pastevoxelmode)
- [Props at target](#doc-builtin-world-editor-targetpropmode)
- [Target cell X](#doc-builtin-world-editor-targetx)
- [Target cell Y](#doc-builtin-world-editor-targety)
- [Target cell Z](#doc-builtin-world-editor-targetz)
- [Props-only target world X](#doc-builtin-world-editor-targetworldx)
- [Props-only target world Y](#doc-builtin-world-editor-targetworldy)
- [Props-only target world Z](#doc-builtin-world-editor-targetworldz)
- [Opaque entity handle](#doc-builtin-world-editor-entityhandle)

Settings are stored in the mod package's `extended.mod.json`. The mod reads them with `shroudforge.settings.get(key, fallback)`. See the [Lua source file](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua) for implementation details.
