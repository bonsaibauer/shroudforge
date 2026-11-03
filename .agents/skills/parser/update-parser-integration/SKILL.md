---
name: update-parser-integration
description: Update or repair the pinned kfc-parser submodule integration and its direct Rust consumers, including schema/snapshot APIs and local compatibility preparation.
---

# Update the Parser Integration

Read `.agents/skills/repository/repository-working-rules.md`. The parser is the direct `kfc-parser/` submodule. Parser-owned crates and implementation stay inside that submodule. Do not introduce `kfc-parser/kfc-parser/`, another vendor tree, or copied parser logic in a feature module.

## Establish the Exact State

Inspect the root gitlink, `.gitmodules`, `git -C kfc-parser status --short`, and `git -C kfc-parser rev-parse HEAD`. Preserve local submodule work. Read `dev/parser/prepare_parser.ps1`, the relevant parser implementations, and all affected direct Cargo consumers. Use `rg` to trace path dependencies and actual `kfc` API calls.

The preparation script recognizes integrated revisions and one legacy revision. Integrated revisions already contain the integration. The legacy path uses `dev/parser/parser-integration.patch`, checks whether it is already applied, then checks forward applicability before applying. Unsupported pins fail deliberately. Read the current script for the exact accepted revisions instead of copying an old revision list into a new workflow.

The recorded legacy patch is broader than an additive schema API overlay. It also changes reader/writer/cache/bounds behavior and removes older tests. Never apply it indiscriminately to an integrated or unknown revision, use `--reject`, reset the submodule to hide a conflict, or overwrite local changes.

## Implement in the Owning Layer

When updating the pin, inspect the new parser diff and the repository consumers together. Maintain the direct Cargo paths and required features. Reconcile `GameFiles`, `GameParser`, `KfcParser`, `GameSchema`, `schema_from_registry`, name-based `kfc_format` resource helpers, and snapshot exports only where actual callers use them.

Reuse freshly extracted executable reflection and the existing loaded registry where the API already owns it. Preserve physical KFC resource identity and serialization details inside the parser boundary. Public schema/snapshot exports use original names and their existing stable fields. Do not add a parallel type cache or expose hashes as a replacement public naming API.

If a new integrated pin is supported, update the preparation script only after verifying that its expected integration is actually included. Keep any retained legacy compatibility route reproducible. A parser change must preserve historical runtime profiles and their interpretation without rewriting their JSON files.

## Verify

Run `pwsh -NoProfile -File dev/parser/prepare_parser.ps1` after the working tree and intended pin are understood. This command may apply the recorded patch only on the recognized legacy revision.

Run the relevant tests from the parser's own manifest and the affected first-party consumer tests. Use the parser's actual package/test names and current features. `cargo test --manifest-path kfc-parser/Cargo.toml --workspace` checks the parser workspace, while the root workspace checks its consumers. Installed-game/ignored tests require the exact inputs and environment documented by their current implementation. Do not infer server support from a client-only test.

For resource writer changes, verify observable preservation of container/chunk directories, unrelated bytes, flush/error behavior, and readback using the existing relevant fixtures. For extraction/schema changes, verify reflection identities, nested fields, error handling, and snapshot output through actual consumers.

Report the old/new submodule pin, local integration handling, parser-owned changes, affected consumers, executed checks, and any tests requiring unavailable game data. Do not claim the whole vendored submodule was audited merely because first-party integration files were reviewed.
