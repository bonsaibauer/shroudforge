# mod-loader

## Purpose

`mod-loader` is the feature-gated facade for the legacy EML loader libraries. It re-exports the shared package layer and optionally exposes Lua and runtime integrations.

## Current status

This crate belongs to the imported EML compatibility toolchain. New ShroudForge package and runtime work belongs in `Shroudforge_Modloader` and `Shroudforge_API` in the repository root.

## Features

| Feature | Default | Adds |
| --- | --- | --- |
| `lua` | No | `mod-loader-lua` as `mod_loader::lua` |
| `runtime` | No | `mod-loader-runtime` as `mod_loader::runtime` |

## Development

```powershell
cargo test -p mod-loader --all-features
```
