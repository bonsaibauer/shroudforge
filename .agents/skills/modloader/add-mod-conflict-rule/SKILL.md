---
name: add-mod-conflict-rule
description: Add a proven version-scoped conflict between mod packages using the existing compatibility rules and shared execution plan. Use for an explicit incompatibility rule, not for dependencies, shortcut conflicts or native game-profile guards.
---

# Add a Mod Conflict Rule

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Establish the two exact manifest IDs, affected SemVer ranges and observed incompatibility. Read `modules/modloader/conflicts/rules.json`, `rules.schema.json`, `check_mod_conflicts.rs` and `plan_mod_load_order.rs`.

Add the rule to the existing `conflicts` array with `first`, `second` and a useful `reason`. Each side has its own ID and version requirement. Avoid broadening the affected versions beyond the evidence.

`check_mod_conflicts.rs` loads the configured `compatibility/rules.json` when present and otherwise uses the embedded rules. It validates the document and blocks both IDs only when both matching packages are enabled. The shared mod plan carries those reasons into startup, runtime candidates and native DLL planning. Keep that single implementation.

Verify matching and nonmatching version pairs, disabled packages, target/dependency interactions and the resulting status explanation using relevant existing checks when requested. Do not install a new user rule file over an existing customized one merely to test the source change. Report the evidence, version boundaries and consumers.
