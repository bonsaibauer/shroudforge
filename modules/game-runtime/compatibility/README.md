# ShroudForge Compatibility

## Purpose

This crate combines parser metadata and declared API operations. Declaring an operation does not prove live readiness: the API/provider checks the current world, entity and individual component on every live access.

## Current status

Approved profiles under `../profiles/enshrouded/` describe their exact client or server build, native addresses, layouts, and separately validated access paths. The native runtime selects and checks the profile. This crate combines the parser schema with declared API operations and does not select a profile or prove live readiness.

Automatic profile selection may use an explicitly opted-in profile after a game update. A profile chosen manually is also tried on a different build. the launcher warns that the build differs and problems may occur. Missing or ambiguous hooks, invalid layouts and incompatible component sizes make the affected operations unavailable. Structural checks do not prove unchanged engine semantics.

## Layout

| Path | Responsibility |
| --- | --- |
| `lib.rs` | Compatibility contract, availability states, and validation |
| `../profiles/` | Authoritative build-profile JSON catalog shipped to the configured installation `runtime/profiles` directory and read by both native selection paths |

## Development

```powershell
cargo test -p shroudforge-compatibility
```

Build-specific data lives in JSON profiles under `modules/game-runtime/profiles/`. Both native selectors use the one installed external catalog. There is no embedded catalog or fallback. Data-only profile changes do not require rebuilding the native runtime. New hook calling conventions require runtime code changes. Incompatible provider ABI changes require a loader update.
