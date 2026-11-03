---
name: create-mod-extension
description: Author extended.mod.json for an existing unpacked EML mod using the current ShroudForge extension format. Use for package metadata, activation, targets, settings or links without extending the schema or creating a whole new mod.
---

# Create a Mod Extension

Read `.agents/skills/repository/repository-working-rules.md`, the package's complete `mod.json`, any existing extension and Lua consumers, `modules/modloader/manifest/extended.mod.schema.json`, and `docs/modding/create-mod-package.en.md`.

Keep the package directly under its existing mod directory. Preserve its EML `mod.json` identity, version, capabilities, entrypoint and execution behavior. The extension adds ShroudForge metadata without replacing the EML manifest. Do not invent a new category, archive reader or settings file.

Distinguish provenance from process scope. `launcher: "EML"` identifies an EML package, while new ShroudForge-authored packages use the existing SF provenance convention. `targets` describes eligible client/server processes. Verify defaults and the current manifest's activation before choosing values, rather than assuming enabled or client-only behavior. Read `ensure_eml_extension.rs`: the canonical first-edit path for an EML package with no extension preserves `manifest.enabled`, uses EML provenance, both client/server targets and an empty settings object. Existing extensions are read directly by `update_mod_configuration.rs`, whose revision-checked update path preserves unrelated metadata.

Use only fields and setting types accepted by the current extension schema. Keep ordinary settings in its `settings` object and use stable Lua keys. Check `read_mod_settings.rs`, `validate_mod_settings.rs`, `validate_mod_manifest.rs`, and the package's Lua settings/actions before adding controls or groups. Add links only using accepted `mod_link_order.json` IDs and HTTPS URLs. Use package-local locale catalogs when visible metadata needs translation.

Validate the combined package through `read_mod_manifest.rs` and the existing package checks. Check that unchanged activation, target and EML execution behavior remains unchanged. When live verification is requested, use the actual intended process and verify settings/actions through its logs, then report what was exercised. A schema evolution belongs to `extend-mod-extension-schema`, and creating a complete package is a separate task.
