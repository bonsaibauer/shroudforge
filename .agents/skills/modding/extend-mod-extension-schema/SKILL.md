---
name: extend-mod-extension-schema
description: Extend the ShroudForge extended.mod.json format across its schema, parser, saved package data and UI/runtime consumers. Use for a new extension field or setting-control kind, not for adding an ordinary setting using the existing format.
---

# Extend the Mod Extension Contract

Read `.agents/skills/repository/repository-working-rules.md` and `docs/modding/create-mod-package.en.md`. Trace the existing extension reader and writer before changing the wire format.

The contract is `modules/modloader/manifest/extended.mod.schema.json`. `EXTENDED_MOD_FIELDS` and `parse_manifest_with_extension` in `parse_mod_manifest.rs` validate and project package-local extension data into `ModManifest`. Internal types live in `mod_manifest_types.rs`. Settings decode in `read_mod_settings.rs`, validate in `validate_mod_settings.rs`, and cross-field relationships validate in `validate_mod_manifest.rs`.

Update the schema, top-level allowlist, projection, internal types and actual consumers together. For a new setting-control kind, also trace `modules/modloader/ui/mod-types.ts` and `edit-mod-setting.tsx`. Update package locales only when the new feature has translatable metadata. `mod-locale.schema.json` is a separate contract.

Preserve activation and process-target defaults. `ensure_eml_extension.rs` supplies EML provenance only when an EML package has no extension. `update_mod_configuration.rs` is the canonical revision-checked writer used by `save_mod_settings.rs` and `set_mod_enabled.rs`. It validates the complete manifest before writing the package extension. Keep settings in that package file and preserve unrelated metadata. Do not infer a new storage path from the generic document descriptor.

Inspect existing examples and shipped packages before updating them. Document the field, defaults, validation and whether it applies live or at restart. Verify through the real parser/writer and relevant existing `shroudforge-package` checks when requested. Report compatibility with extensions missing the new optional field and with legacy EML packages missing the whole extension.

`docs/website/tools/build-website-output.mjs` publishes this canonical schema. Follow affected examples, independent validation and control rendering under `docs/website/page-integrations/website-manifests/`. Update the corresponding English and German articles under `docs/pages/creating-mods/extended-mod-json/`. Never edit generated schema copies or rewrite historical game-build runtime profiles as part of this separate contract.
