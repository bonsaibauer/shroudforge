---
name: extend-mod-manifest-schema
description: Extend the EML mod.json contract and its actual readers, internal model and consumers. Use for a new manifest field or capability, not for authoring one package or adding ShroudForge settings to extended.mod.json.
---

# Extend the EML Mod Manifest

Read `.agents/skills/repository/repository-working-rules.md`, `docs/development/code-ownership.en.md`, and `docs/modding/create-mod-package.en.md`. Preserve the existing working tree.

Trace the proposed field from package input to its observable consumer before editing:

- `modules/modloader/manifest/manifest.schema.json` defines the accepted EML wire document.
- `modules/modloader/parse_mod_manifest.rs` owns `MOD_FIELDS`, schema normalization and projection into the internal manifest.
- `modules/modloader/mod_manifest_types.rs` owns `ModManifest`, `Capability`, dependency types and execution predicates.
- `modules/modloader/read_mod_manifest.rs` applies data-only adapters before parsing and infers scheduling without granting permissions.
- `modules/modloader/validate_mod_manifest.rs` owns cross-field validation. `find_installed_mods.rs` discovers unpacked installed packages.

Add the field to every required layer. A schema-only addition can be discarded by `MOD_FIELDS`, and the internal model rejects unknown fields. Give optional additions compatible defaults. Preserve existing EML names, capability declarations, process scope, dependency ordering and failure behavior. Add feature semantics in the owning module rather than the parser.

Follow actual consumers into UI snapshots, Lua registrations, startup planning, installation and packaging when the new field changes them. Review `create_mod.rs`, compatibility overlays in `mod_profiles.rs`, package examples and documentation when their output should contain the field. Do not move settings or activation into `mod.json`.

`docs/website/tools/build-website-output.mjs` publishes the canonical schema into generated website output. The independent example, renderer and validation consumers under `docs/website/page-integrations/website-manifests/` must follow affected public fields. Update the matching English and German field articles under `docs/pages/`. Do not edit generated schema copies. Historical game-build runtime profiles remain immutable and are unrelated to this mod manifest contract.

Use the package reader for verification. Relevant existing tests live inline in the Modloader crate, whose Cargo package is `shroudforge-package`. Run the appropriate requested checks, and report compatibility/default behavior and any unverified runtime effect.
