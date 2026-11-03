---
name: change-blueprint-persistence
description: Change World Editor blueprint encoding, validation, rotation, import/export, library file operations, or screenshot cover management while preserving the existing SFBP and native/Lua ownership contracts.
---

# Change blueprint persistence

Read `.agents/skills/repository/repository-working-rules.md`. Identify whether the request changes the blueprint format, library operations, transfer compatibility, or local cover files. Read every current reader/writer before changing persisted data.

## Canonical data owners

- `mods/world-editor/src/blueprint.lua::{encode_blueprint,load_blueprint_content}` owns SFBP V7 and coordinate/rotation calculations.
- `library.lua` owns unique capture naming, file export/load, library refresh, and coordinated rename/duplicate/delete.
- `operations.lua` creates captures and consumes plans. `history.lua` retains runtime undo, which is distinct from a saved blueprint.
- `p2p.lua` transfers the same encoded body. Complete validated server captures are saved by the existing client codec/library path.
- `modules/world-editor/blueprint_library.rs` discovers native cards/state and validates names.
- `modules/world-editor/blueprint_covers.rs` owns local screenshot folders, images/thumbnails, backup, and undo. `ui/run_blueprint_action.rs` dispatches those local file actions.
- `ui/index.html` and `ui/fallback.rs` are both current library/manager consumers.

## Preserve compatibility and complete file sets

1. Keep or deliberately version the canonical format with explicit reader/writer compatibility. Preserve voxel coverage (unknown/empty/occupied), cell order, dimensions, original cell size/origin relationship, world-space anchors, quarter-turn rotation, exact hexadecimal UUID halves, and nonuniform prop scale. Never cast UUID halves through floating-point numbers.
2. Validate current placement recipes, finite transform data, dimensions/counts/file limits, names, and complete channel metadata before activating imported data. Native handles are process-local and cannot become portable identities.
3. Preserve the distinct 256 MiB local blueprint and 32 MiB online transfer limits. Framing/checksums do not replace blueprint semantic validation. Save no incomplete/corrupt/stale download.
4. Use configured export paths and non-overwriting capture names. Preserve coordinated blueprint/PNG/thumbnail rename or duplicate rollback and explicit deletion results. Do not introduce a second blueprint index or route registry when directory discovery is the existing owner.
5. For covers, retain canonical source-folder containment, supported image format/content checks, file/dimension/pixel limits, bounded screenshot listing, generated PNG/thumbnail sizes, and prior-cover undo. Keep cover processing in the native file owner rather than an in-game world API.

## Verify

`modules/mod-runtime/tests/world_editor_lifecycle.lua` contains an independent literal V7 file, rotation expectations, exact UUID/scale preservation, and real shipped Lua/online capture fixtures. Extend those only for meaningful changed compatibility and run `cargo test -p shroudforge-api --test bundled_lua`. Rebuild `shroudforge-world-editor-ui` through the existing product build for native UI/file changes. Inspect the affected import/export, card/manager, image/undo, and transfer behavior in the authorized live environment. Distinguish fixture compatibility from native world effects and save persistence.
