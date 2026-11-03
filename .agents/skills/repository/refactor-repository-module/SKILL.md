---
name: refactor-repository-module
description: Refactor or reorganize ShroudForge source while preserving behavior, canonical module ownership, callers, persistence, dynamic loading, build inputs, and package paths. Use for structural code changes rather than a new domain feature.
---

# Refactor a Repository Module

Read the shared `.agents/skills/repository/repository-working-rules.md` and `docs/development/code-ownership.en.md`. Establish the requested structural change and preserve the starting working tree.

Before moving or splitting code, read its complete implementation and trace direct and dynamic callers, configuration readers/writers, build scripts, tests, and release packaging. A failed text search does not prove a file is unused.

Keep each action with its owning module. Let UI, CLI, and background entrypoints call that owner instead of duplicating behavior. Keep module-specific UI with its feature. Use shared UI resources only for demonstrated shared consumers.

For every new, moved, or split file, record the concrete action, owner, full destination, descriptive name, callers, and why an existing file does not already own it. Avoid vague collections such as `misc`, `helpers`, `utils`, and `common`. Do not split solely by line count or create empty forwarding layers.

## Repository Boundaries

- Mod packages are direct children of `mods/<mod-id>/`. Add no invented package categories.
- The parser and its crates belong inside the direct `kfc-parser/` submodule. Add neither `kfc-parser/kfc-parser/` nor a new vendoring layer.
- Do not add an intermediate `src/` unless the actual language/build system requires it.
- Choose native/runtime directories from demonstrated responsibilities and callers.
- Website articles and integrations retain their separate source discovery and generated-output contracts. Use their task skills when those sources move.
- Development actions use the existing `dev/` entrypoints. Keep each new action concrete and reusable.

## Preserve the Contract

Retain behavior, interfaces, serialized formats, sequence, errors, locks, cooldowns, transactions, and release paths unless the user requested a specific change. Keep a single configuration format and canonical writer.

Installed mods remain unpacked folders. Archive extraction stays in installation. EML extension creation remains separate from configuration persistence. Historical game-build profiles remain immutable and must not be rewritten to accommodate a refactor.

Do not alter local user data, logs, settings, recovery files, profiles, or source build inputs as cleanup. Follow dynamic registrations, generated declarations, asset inclusion, and package scripts before deleting an old path.

Check the changed callers and entrypoints through appropriate existing build/check actions. Compare the full diff with the starting behavior and report concrete preserved boundaries and any unverified ones.
