# ShroudForge Repository Working Rules

Paths in the repository skills are relative to the repository root unless a link explicitly says otherwise.

## Establish the Change

- The active task owner remains responsible for understanding the current implementation, selecting owners, making the authorized change, and checking the result. Supporting research does not transfer that responsibility. Do not transfer task ownership to a supporting agent unless the user explicitly requests it.
- Check `git status --short` before editing and preserve all pre-existing changes.
- Use `docs/development/code-ownership.en.md` for current ownership. Material under `docs/evidence/` records dated investigations and is not an alternative ownership map.
- Read affected implementations completely and trace their actual callers, data flow, persistence, build entrypoints, packaging, and dynamic discovery. Distinguish observed behavior, requested behavior, a proposal, and an unresolved question.
- Before creating or moving a file, establish its action, owning module, full target path, descriptive name, callers, and why an existing owner does not already perform the action.

## Preserve Ownership and Behavior

One action has one canonical implementation. UI, CLI, and background callers supply inputs and present results. Each feature module owns its behavior and UI. `modules/transport/` is a module. Share UI resources only between actual consumers.

Trace all readers and writers before changing a configuration contract. Reuse its current storage and canonical writer. Keep installed data, settings, logs, profiles, captures, exports, and build inputs intact unless the task establishes their owner and intended use.

For a behavior-preserving change, retain public interfaces, formats, ordering, errors, locks, cooldowns, transaction boundaries, and package paths. The `refactor-repository-module` skill contains the detailed structural workflow. Historical game-build profiles are immutable. Create a separate file for a new build and verify the baseline hash before and after. Never bulk-rewrite old profiles to accommodate schema or runtime changes. Domain skills retain their additional constraints.

Use existing concrete entrypoints under `dev/`. Add only a demonstrated reusable action, without inventing a generic tool or job framework.

## Check and Communicate

Use existing checks and build actions appropriate to the authorized change. Read the full resulting diff. Report the owner, callers, behavior boundaries, checks actually performed, failures, and remaining unverified behavior. For a complete source audit, use `audit-repository-code` and record its exact scope and reading coverage.

Default user-facing communication to German with direct umlauts and ß unless the user requests another language. Do not use semicolons in German prose. Preserve required code/configuration characters. Repository skill instructions remain in English.

A skill is a reusable task procedure. Its files do not themselves create a ChatGPT agent window or a GitHub Actions workflow.
