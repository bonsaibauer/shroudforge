---
name: add-native-engine-operation
description: Implement an evidence-backed native engine operation using original registrations, decoded code contracts, owned inputs, and existing world lifetime guards. Use for new native gameplay operations rather than address-only profile updates.
---

# Add a Native Engine Operation

Read `.agents/skills/repository/repository-working-rules.md`. Establish the requested operation, its input/output contract, owning native domain, current callers, and exact executable evidence. Keep static candidate discovery, ABI proof, world admission, submission, and observed completion separate.

## Prove the Original Route

Read the nearest complete production implementation and its tests. Existing examples are:

- `native/impact/compare_entity_ids.cpp` and `resolve_registered_operation.cpp` for exact named registered callbacks on owned primitive VM inputs.
- `native/impact/submit_entity_destruction.cpp` and `native/ecs/resolve_entity_command_queue.cpp` for an original queued mutation.
- `native/impact/submit_building_prop_spawn.cpp` for a named original system, decoded transitive instruction roles, owned inputs, and an exact submission receipt.
- `native/voxels/read_registered_voxel_grid.cpp` and `write_registered_voxel_grid.cpp` for registered services, original storage/commit roles, complete chunk preservation, and verified readback.
- `native/code-inventory/resolve_original_call_graph.cpp` for original instruction boundaries, chained unwind ranges, reachable code selection, and retained code evidence.

All paths above are under `modules/game-runtime/`. Resolve complete original operation/service/system names and retain ambiguity. Hash equality, byte proximity, a unique AOB, or a matched function entry does not prove arguments, a hook site, lifetime, or gameplay effects. Reuse the existing decoder and code-evidence helpers. Do not introduce an operand-byte CALL scanner or a separate RVA catalog for a dynamically resolved operation.

## Implement the Admission and Effect Contract

Use the current `EcsRuntime::SessionGuard`, original thread-registry lifetime, and `EngineContext::EcsWriteGuard` contracts as the operation requires. Follow their lock order and nonblocking acquisition behavior. Confirm authority, current owner, entity membership/generation, and exact binding again at the established boundaries.

Prepare owned buffers, contracts, storage, and host-side locks before entering original engine gates. Borrowed engine pointers must not escape their valid guard scope. Reusing a service binding or registered function pointer does not authorize an arbitrary call ABI.

For a mutation, define its last safe rejection point, publication point, and confirmed effect. Validate original code evidence and complete expected state before publication. After publication, preserve the existing distinction among rejection, submission, confirmed completion, conflict, stale session, and uncertainty. Never retry or switch mutation mechanisms after an uncertain published outcome. Fence the affected session through the existing uncertainty mechanism. Roll back only when the original operation contract proves it safe and verify the complete restored state.

Reuse canonical ECS/resource writers for supported field operations. Their component tracking, full-snapshot comparison, scalar alignment, original page-protection restoration, and uncertainty fences are part of the behavior. A successful field write does not establish a gameplay effect or persistence.

Expose the operation through the existing ABI and Rust bridge. Use `extend-native-runtime-abi` and `extend-runtime-provider-adapter` only for the layers actually changed. Preserve all historical profiles and their interpretation. A new build requires a new same-target profile copy, with the baseline hash unchanged before/after.

## Verify

Build with `pwsh -NoProfile -File dev/build/build_kfc_runtime.ps1`. Extend the nearest existing private fixture to test observable success, malformed input, ambiguity, stale owners/sessions, output nonpublication, and postpublication failure. Relevant explicit CMake targets include `kfc-runtime-test-original-code`, `kfc-runtime-test-entity-command-queue`, `kfc-runtime-test-entity-destruction`, `kfc-runtime-test-native-mutation-contract`, and `kfc-runtime-test-world-context`.

Read each test's command-line contract before running it. The native mutation contract test maps an exact original EXE as data and requires its analysis RVA. It makes no live world available. Private fixtures and isolated emulation prove their stated boundary only.

When the user requests live checking, verify the actual operation through its real caller in the correct process and authorized test world, inspect debug logs, and retain readback evidence. Record dispatch, observed world effect, and save/reload persistence separately. Preserve recovery evidence and stop dependent mutations after an uncertain outcome.
