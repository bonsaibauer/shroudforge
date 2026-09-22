# ShroudForge Compatibility

## Purpose

This crate turns parser output into a verified game contract. It is the boundary between data that can be discovered in Enshrouded files and operations that ShroudForge is willing to expose safely.

## Current status

The current Windows client profile targets Enshrouded build `1076226`. The profile records the expected runtime operations, component layouts, and native addresses used by the runtime bridge.

Unsupported builds must not silently reuse this profile. A new game build requires a new or reviewed profile before runtime features are considered compatible.

## Layout

| Path | Responsibility |
| --- | --- |
| `src/lib.rs` | Compatibility contract, availability states, and validation |
| `windows/enshrouded_client_1076226.h` | Native Windows profile for the supported client build |

## Development

```powershell
cargo test -p shroudforge-compatibility
```

Update the Rust contract and native header together when a supported runtime operation or game build changes.
