---
name: extend-native-runtime-abi
description: Extend or revise the C ABI between kfc-runtime.dll, the Rust runtime bridge, and the Windows bootstrap. Use when exports, native records, ABI versions, or provider symbol requirements change.
---

# Extend the Native Runtime ABI

Read `.agents/skills/repository/repository-working-rules.md`. Establish the exact operation, its native owner, Rust callers, and bootstrap consumers before changing the ABI. A new Lua API entry alone belongs to the Lua API workflow. A new game build alone belongs to the client or server profile workflow.

## Trace the Contract

Read these affected files and their actual callers completely:

- `modules/game-runtime/native/abi/runtime_api.h`, the canonical exported C declarations and fixed-width records.
- `modules/game-runtime/bridge/windows/native_abi.rs`, the Windows function pointer types and `Provider` fields.
- `modules/game-runtime/bridge/runtime_records.rs` for shared `repr(C)` records, and `bridge/lib.rs` for the bridge version contract.
- `modules/game-runtime/bridge/windows/provider/load_native_provider.rs`, including its required and optional symbol macros and cached load results.
- `modules/entrypoints/windows-bootstrap/runtime_bridge.cpp` when lifecycle exports or the ABI version are affected.
- The native implementation, Rust operation adapter, relevant unsupported-platform implementation, and native CMake source/test lists.

Use a fixed-width C representation with the existing calling convention. Specify sizes, alignment, ownership, string encoding, NUL termination, count/capacity units, output publication, and every result value. Preserve existing numeric results and optional-symbol behavior unless the task explicitly changes them. Never pass Rust-owned strings, containers, exceptions, or C++ objects across this boundary.

## Implement Both Sides

Implement the operation once in its existing native domain. Add the C export and its matching Rust signature, record representation, and provider lookup. Change the ABI version consistently in all consumers when compatibility changes. Required exports make older providers unavailable. Optional exports require an explicit unsupported result in their Rust adapter. Choose deliberately based on the requested compatibility contract.

The provider loader uses `GetModuleHandleW` and does not load the DLL. Preserve startup ownership and the distinction between a provider that is not loaded yet and a permanent ABI/export error. For JSON exports, reuse `read_native_catalog.rs` and its complete-buffer retry/NUL convention instead of parsing a truncated prefix.

Keep historical runtime profile files byte-for-byte unchanged, including during schema or ABI evolution. Preserve their interpretation through compatible readers or explicitly versioned new contracts. For a new build, use a separate copied profile and the applicable profile skill, with the baseline hash verified before and after.

## Verify

Build through `pwsh -NoProfile -File dev/build/build_kfc_runtime.ps1`. It configures `build/native-runtime`, builds Release, and copies the provider to `build/x64/kfc-runtime.dll`.

Run the relevant Rust package checks, for example `cargo test --release -p shroudforge-runtime-bridge`. Add or extend a meaningful private ABI fixture when the change affects layout, capacities, failure publication, or status mapping. Build the affected native test targets explicitly because they are excluded from the default build.

`build/native-runtime/tools/kfc-runtime-dev.exe inspect-provider <dll>` checks only its implemented required export set. Compare the changed exports and required/optional bridge lookups independently. A successful DLL load or ABI check does not verify gameplay behavior.

Report the ABI change, native and Rust consumers, compatibility behavior, builds/tests actually run, and any live behavior still unverified.
