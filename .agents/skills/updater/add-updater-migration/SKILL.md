---
name: add-updater-migration
description: Add a versioned updater maintenance or migration job to the existing queue lifecycle, using declarative setting updates when sufficient. Use for installed-data migration, not for runtime diagnostic jobs or editing a live installation during development.
---

# Add an Updater Migration Job

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Read `modules/updater/maintenance_jobs.rs`, the nearest migration JSON, queue execution/storage and the owning data module. Establish the exact legacy input, preserved user data, idempotent transformation, trigger and completion evidence.

Use the existing `JobDefinition`: schemaVersion, stable migration./maintenance. ID, positive revision, name, description, before-update/after-install trigger, repeatable flag and supported action. Definitions are at most 16 KiB. Add a bundled default to `DEFAULT_JOBS` when it must ship. Required migrations also belong in the existing `REQUIRED_MIGRATIONS` check with an explicit minimum revision.

For changes to existing module settings, use `update-loader-settings` and declarative `settings` entries. Use `previousValue` when customized user values must survive. A new Rust action is appropriate only when the migration needs conversion, domain validation or other behavior that the declarative action cannot express. Implement that action in the module that owns the data and adapt it through the existing executor.

Preserve queue IDs/revisions, superseded-job behavior, cancellation and failed/retry semantics. Installed job files are user data and existing defaults are not generally overwritten. If a shipped definition revision changes, trace whether an explicit upgrade of the installed descriptor is required. After-install jobs are tied to installation receipts, and hotkey conversion precedes settings validation.

Use temporary fixture installations for verification. Verify already-current input, legacy input, repeated execution, partial failure, preservation and recovery. Do not run a migration against the user's real configuration as an authoring test. Report the ID/revision, trigger, exact data transformation and relevant existing checks executed.

Historical game-build runtime profiles are immutable references. A migration or schema change must not rewrite, normalize, rename or delete those files. Mod compatibility adapters have a separate owner and update contract.
