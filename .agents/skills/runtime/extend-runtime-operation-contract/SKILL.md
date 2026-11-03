---
name: extend-runtime-operation-contract
description: Extend a versioned runtime operation identifier and its phase, capability, compatibility, and live availability contract. Use when runtime.has, require, status, or operation enumeration must admit new behavior.
---

# Extend a runtime operation contract

Read `.agents/skills/repository/repository-working-rules.md`. First identify the exact operation and the requested client/server/phase behavior. Separate declaration, implementation, readiness, authorization, dispatch, and confirmed effect.

## Trace the contract

Read these current owners and their relevant callers completely:

- `modules/mod-runtime/lua/shroudforge/v1/runtime-operations.json` and `runtime-operations.schema.json`.
- `lua/shroudforge/v1/read_runtime_operations.rs`, `modules/mod-runtime/documents.rs`, and `modules/mod-runtime/lib.rs` for validated API data.
- `modules/mod-runtime/session/create_mod_session.rs` and `modules/game-runtime/compatibility/lib.rs` for session compatibility.
- `modules/mod-runtime/lua/ingame/check_ingame_access.rs` and the actual API factory/feature module.
- The existing Rust bridge/native operation and exact profile evidence when this reaches Enshrouded.

`check_ingame_access.rs` uses explicit operation admission and rejects unrecognized operations. Do not make an unknown identifier available merely by adding it to the JSON document.

## Implement consistently

1. Add the operation to the canonical versioned document with the existing schema version and validation conventions. Change the schema only if the data shape actually changes.
2. Implement or route the operation through its current domain owner. `lua/ingame/functions/mod.rs` reuses checked wrappers and explicit aliases for the function catalog. It is not a generic native function invoker.
3. Update availability for the operation with the existing phase/capability/provider/lifecycle checks and operation-specific context. Preserve role checks, session generation, authoritative write scope, write fault handling, and informative denial reasons.
4. Keep `runtime.get_operations`, `runtime.has`, `runtime.require`, and `runtime.status` consistent. A transient busy original gate is different from an unimplemented operation. Readiness observations do not reserve a gate or authorize retry after uncertain dispatch.
5. Update public Lua definitions under `lua/shroudforge/v1/definitions/runtime_api.lua` as needed, using the same source for generated website/editor documentation.

When a new native operation or profile contract is needed, complete that owner workflow and its exact executable verification. Inventory names, reflected types, and scheduler membership do not establish an ABI or a safe original call.

## Verify the boundary

Run relevant runtime/bridge checks and `cargo test -p shroudforge-api`. Include unavailable, wrong-phase/capability/role, ready, and changed-session cases where they affect behavior. Run `./dev/build/check_website.ps1` if public declarations changed. Test the requested operation in the actual client or server when the task calls for native verification, retaining logs/readback and distinguishing dispatch from effect. Preserve incomplete/unknown outcomes in the report.
