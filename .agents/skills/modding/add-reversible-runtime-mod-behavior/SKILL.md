---
name: add-reversible-runtime-mod-behavior
description: Add or change a Lua mod effect that temporarily changes live ECS attributes, components, templates, services, or loaded resource fields and must restore its own original inputs.
---

# Add reversible runtime mod behavior

Read `.agents/skills/repository/repository-working-rules.md`. Identify the original game input, authoritative target, setting/default, desired calculation, and observable effect. Read the entire existing mod and the public writer/override target before changing it. Do not invent a memory offset or gameplay formula from a label.

## Reuse the runtime ownership contract

`modules/mod-runtime/lua/ingame/overrides/track_runtime_overrides.lua` owns baseline tracking and recovery. `create_override_targets.lua` supplies checked attribute/component/template/service/resource adapters. Rust `track_runtime_overrides.rs` owns the per-mod registry and lifecycle cleanup. `claim_runtime_storage.rs` coordinates ownership of original storage across mods. Use `runtime.overrides.new()` and the existing target factories rather than a second restore system.

Bundled references are `mods/sf-survival/src/mod.lua`, `mods/sf-unlimited-flight/src/mod.lua`, and `mods/sf-crafting-essentials/src/mod.lua`. Choose the relevant storage type rather than copying unrelated feature logic.

## Implement the effect

1. Declare the setting in the existing package extension with the correct type/range/default, target, group, and live/restart behavior. Use `shroudforge.settings.get` for applied values.
2. In lifecycle updates, begin an override update, restore disabled settings, check the tracker fault, and wait for the required world/operation availability. Avoid needless repeated writes and unbounded work.
3. Resolve the original target through checked APIs. Compute the desired value from the captured original baseline and current validated dependencies, not an already modified live value. Normalize using the original scalar representation.
4. Apply through the target adapter. Preserve compare-before-write, expected dependent fields, session identity, claim acquisition/release, and fresh snapshots. Shared templates/resources require shared-storage reasoning. Player-specific intentions cannot safely control a shared input without the established constraint.
5. Preserve restoration on disable, unload, normal end of use, and failed activation. The loader automatically cleans registered trackers before releasing the scope. If custom non-override state needs cleanup, implement it in the existing unload callback.

A busy gate retains a retryable baseline. External changes are preserved. An uncertain write or rollback faults the tracker and blocks later apply/restore writes. Do not clear that fault or discard unresolved ownership to make a setting appear successful. Session 0 during menus/loading does not prove the previous world ended. A known new nonzero generation must never authorize restoration through an old target.

## Verify

Extend meaningful shipped-mod fixtures under `modules/mod-runtime/tests/` when behavior changes. Cover enabled/default/disabled/unload behavior and the relevant busy/external-edit/session/uncertainty cases. Run the owning explicit target (`flight`, `survival`, `runtime_overrides`, or `bundled_lua`) and broader runtime tests if necessary. These fixtures check public contract/control flow, not the game's original calculation or visible gameplay. Verify the requested effect live on the intended client/server and report readback, gameplay observation, restoration, and remaining uncertainty separately.
