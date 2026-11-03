---
name: integrate-native-mod-dll
description: Integrate a native mod DLL through ShroudForge package selection or EML runtime registration while preserving the single process-owned ordered DLL executor and DirectInput chain contract.
---

# Integrate a native mod DLL

Read `.agents/skills/repository/repository-working-rules.md`. Identify the package, target, DLL path, expected exports, selection source, and current detection/load behavior before changing it. Native library selection and execution are separate owners.

## Trace existing integration

`modules/modloader/dll_manager.rs` owns native component detection and `resolve_load_plan`. Follow its actual package/INI/profile/manual readers, exclusions, ordering, and configuration writer. `modules/mod-runtime/native_dlls.rs` consumes that plan through one process-owned worker, deduplicates canonical paths, and retains handles until exit. `modules/mod-runtime/lib.rs` exposes bootstrap preparation. `session/native_plugins.rs` owns package-local EML `loader.runtime.register_dll` requests.

Read all affected readers/writers and host callers in full. Distinguish original game-build profiles from native-mod compatibility adapters under `modules/modloader/mod-profiles/`.

## Implement at the right owner

1. Add or correct package/adapter declaration or selection logic in the existing Modloader path. Preserve target, activation, exclusions, manager enablement, dependencies, and load mode. Do not add another configuration file for the same choice.
2. Keep registration package-local and use the canonical path resolver. Preserve existing EML phase and package restrictions without inventing an unrelated capability requirement.
3. Route every selected or registered DLL through the existing native executor. Preserve ordered execution, case-insensitive canonical deduplication, mode consistency, bounded bootstrap wait, and per-entry error reporting.
4. For DirectInput, follow the installed chain host and exact export contract, including the existing `DirectInput8Create` requirement where applicable. Do not bypass the chain by calling LoadLibrary from a Lua binding or UI.
5. Preserve handles until process exit. Lua unload/reload does not unload a native DLL. Selection changes that require restart must remain explicit and must not pretend to replace an already loaded module.

## Verify

Run relevant package/load-plan and runtime executor tests and rebuild the affected product/bootstrap with existing `dev/build/` actions. For the requested live target, record the effective plan, actual path/mode/order, load result/errors, duplicate behavior, and functional native-mod evidence if available. Loading proves that the DLL was loaded, not that its hooks or gameplay behavior work. Do not repeatedly load a second copy to recover an uncertain library state. Report restart or missing native dependencies as concrete limits.
