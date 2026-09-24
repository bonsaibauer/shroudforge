# ShroudForge Compatibility

## Purpose

This crate combines parser metadata and declared API operations. Declaring an operation does not prove live readiness: the API/provider checks the current world, entity and individual component on every live access.

## Current status

The current Windows client profile targets Enshrouded build `1076226`. The profile records the expected runtime operations, component layouts, and native addresses used by the runtime bridge.

An explicitly opted-in profile may undergo structural revalidation after a game update instead of being rejected solely for a changed build identity. Missing/ambiguous hooks, invalid layouts and incompatible component sizes produce scoped failures. Structural checks do not prove unchanged engine semantics.

## Layout

| Path | Responsibility |
| --- | --- |
| `src/lib.rs` | Compatibility contract, availability states, and validation |
| `../Shroudforge_Modloader/kfc-runtime/compatibility/profiles/` | Authoritative native build profiles; embedded into `kfc-runtime.dll` during its CMake build |

## Development

```powershell
cargo test -p shroudforge-compatibility
```

Build-specific data lives exclusively in the runtime repository's JSON profiles. Changing a profile does not require rebuilding the modloader. New hook calling conventions require runtime code changes; incompatible provider ABI changes require a host update.
