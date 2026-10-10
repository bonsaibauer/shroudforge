# ShroudForge Compatibility

## Purpose

This crate combines parser metadata and declared API operations. Declaring an operation does not prove live readiness: the API/provider checks the current world, entity and individual component on every live access.

## Current status

The current Windows client profile targets Enshrouded build `1076226`. The profile records the expected runtime operations, component layouts, and native addresses used by the runtime bridge.

Automatic profile selection may use an explicitly opted-in profile after a game update. A profile chosen manually is also tried on a different build. the launcher warns that the build differs and problems may occur. Missing or ambiguous hooks, invalid layouts and incompatible component sizes make the affected operations unavailable. Structural checks do not prove unchanged engine semantics.

## Layout

| Path | Responsibility |
| --- | --- |
| `src/lib.rs` | Compatibility contract, availability states, and validation |
| `../runtime/profiles/` | Authoritative native build profiles. embedded into `kfc-runtime.dll` during the workspace CMake build |

## Development

```powershell
cargo test -p shroudforge-compatibility
```

Build-specific data lives in JSON profiles under `src/loader/runtime/profiles/`. Changing a profile requires rebuilding the integrated native runtime. New hook calling conventions require runtime code changes. incompatible provider ABI changes require a loader update.
