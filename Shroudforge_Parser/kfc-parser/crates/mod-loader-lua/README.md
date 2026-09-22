# mod-loader-lua

## Purpose

`mod-loader-lua` hosts the legacy EML Lua environment used to inspect, modify, and export Enshrouded assets outside the running game.

## Current status

This crate belongs to the imported compatibility toolchain. ShroudForge uses its own API host under `Shroudforge_API` for current `1.0.0` mods.

The `definitions/` directory describes the legacy Lua surface. The implementation includes game assets, buffers, images, hashing, I/O, caching, and mod runner support.

## Development

```powershell
cargo test -p mod-loader-lua
```
