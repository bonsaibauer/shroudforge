---
name: extend-runtime-provider-adapter
description: Add or change a Rust adapter for an existing native runtime operation, including provider loading, owned buffers, result translation, and platform fallback. Use under modules/game-runtime/bridge/.
---

# Extend a Runtime Provider Adapter

Read `.agents/skills/repository/repository-working-rules.md`. Trace the existing native export to its Rust caller before deciding whether this is an adapter change or a native ABI extension. Keep engine behavior in its canonical native operation and public Lua binding in its existing mod-runtime owner.

## Follow the Existing Route

Read `modules/game-runtime/bridge/lib.rs`, `windows/mod.rs`, the affected domain adapter, `windows/native_abi.rs`, and `windows/provider/load_native_provider.rs`. Follow the Rust caller and corresponding declaration in `modules/game-runtime/native/abi/runtime_api.h` into the implementation. Check `unsupported_platform.rs` and `building_input.rs` where applicable.

Use the existing domain directory such as `windows/ecs/`, `resources/`, `named_services/`, `impact/`, `props/`, `building/`, `voxels/`, or `world_context/`. Name a new file after the concrete operation. Reuse an existing adapter if it already owns the action.

## Preserve Meanings Across the Boundary

- Convert inputs into owned, bounded buffers and checked native integers. Retain input storage for the entire FFI call. Preserve exact qualified names and resource identities.
- Reuse `read_native_catalog.rs` for complete native JSON exports. Follow its count/retry/NUL contract rather than accepting partial JSON.
- Preserve count-query and fill semantics, pending results, unavailable results, stale sessions, conflicts, and uncertain mutations as distinct outcomes. A zero count is not interchangeable with a failed or pending query.
- Follow `read_world_session.rs` and the relevant session-specific native call. Verify the session around calls when the existing operation requires it. A local numeric entity ID is not a reusable opaque handle.
- Treat an optional missing export as unsupported without bypassing the native admission checks. Do not reinterpret it as an empty successful read.
- A submitted command is not completion. Preserve existing exact entity/readback checks and outcome mapping. Never retry an uncertain published write or substitute a different mutation path.

The bridge obtains an already loaded provider. Do not add independent DLL loading, hook setup, memory scanning, or profile selection to an adapter. Do not expose raw pointers, arbitrary native invocation, or generic patch primitives as a shortcut.

Historical profiles remain unchanged. New profile interpretation belongs to a compatibility contract change, while new build addresses belong to a separately copied client/server profile.

## Verify

Use `cargo test --release -p shroudforge-runtime-bridge` for relevant bridge tests, plus the actual downstream package tests when its public behavior changes. Windows-specific code must be checked on Windows. Use the existing native build and affected private fixture when native result meanings or buffers are involved.

If live verification is requested, exercise the actual caller in the correct client/server process, retain the executable identity and session in evidence, and inspect provider/runtime logs. Do not call a compilation check a live result.

Report the native export, adapter, downstream caller, platform behavior, and each status or buffer invariant checked.
