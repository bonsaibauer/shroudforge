# kfc-content

## Purpose

`kfc-content` builds on `kfc-base` to interpret binary content stored in Enshrouded data files.

## Current status

The crate contains support for images, audio, mesh data, localization, content hashes, and Impact program structures. Format coverage depends on the game data and tests available to the parser project.

## Main modules

- `image` decodes and encodes supported texture formats.
- `audio` handles supported audio content.
- `mesh` contains mesh and PBR structures.
- `impact` models Impact graphs, nodes, and bytecode.
- `localization` handles localization content.

## Development

```powershell
cargo test -p kfc-content
```
