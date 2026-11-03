---
name: add-loader-setting
description: Add one loader or built-in module setting through the existing defaults, schema, canonical persistence, UI snapshot and actual runtime consumer. Use for loader-config settings, not for mod-owned extended.mod.json values.
---

# Add a Loader Setting

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Trace the owning module's actual configuration reader and use of the value. Read `modules/loader-config/loader.default.json`, `loader.schema.json`, `read_loader_settings.rs`, `save_loader_settings.rs`, `validate_document.rs`, and `resolve_installation_paths.rs` where relevant.

Add the compatible default and schema definition in the existing document. Preserve the merge behavior, migrations, installation lock and atomic read-validate-write operation in `config::update_loader`. Generated state belongs in its existing state section rather than user preferences. Do not add parallel files for a setting already owned here.

For a user-editable setting, follow the actual command path through `modules/modloader/ui/host/save_loader_setting.rs` and, where applicable, bulk `save_loader_settings.rs`. Update the explicit allowlists, input normalization and owning consumer together. Extend the Rust snapshot structures/readers and `modules/loader-config/ui/loader-settings.ts` only where a typed field is needed.

Put the feature's UI beside its module. The existing `show-module-settings.tsx` composes those controls. Use existing shared setting controls, host commands and translations. Trace restart, live refresh, disabled-module and error behavior before choosing labels.

If changing saved existing values is required, use the updater migration workflow rather than rewriting user installations during source editing. Preserve user-customized values unless migration scope explicitly requires otherwise. Verify default loading, missing-field compatibility, invalid values, save paths and actual consumption with the relevant requested checks.
