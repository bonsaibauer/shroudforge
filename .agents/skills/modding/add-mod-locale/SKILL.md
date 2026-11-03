---
name: add-mod-locale
description: Add package-local translations for a Lua mod's metadata, settings, actions and Lua strings using package-local locale JSON files. Use for mod-owned translations, not for the loader's shared UI catalogs or offline Wiki articles.
---

# Add a Mod Locale Catalog

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Read the package's `mod.json`, `extended.mod.json`, Lua consumers and existing locale catalogs completely. The source text remains in the manifests and Lua fallbacks.

Use `modules/modloader/manifest/mod-locale.schema.json` and the actual reader `read_mod_locale_catalogs.rs`. Catalogs are direct JSON files under the package's `locales/` directory. Locale filenames must satisfy the existing locale-code rules and must not duplicate another code case-insensitively.

Translate only declared fields. Settings and choice options match their source keys. Groups match the source group array order, and action translations match IDs inside the corresponding group. Translated changelog entries cannot outnumber the source. `resolve_mod_locale.rs` owns these relationships and locale fallback. Regional catalogs override the base-language catalog, while missing text retains the source. The legacy `en` lookup prefers `en-GB`.

Place free Lua-facing strings under `strings`, and use the existing `shroudforge.i18n.get` and selected `shroudforge.locale()` API. Follow `docs/examples/starter-mod/src/mod.lua` for a real consumer. Do not turn setting keys or action IDs into translated identifiers.

The package reader validates catalogs and `calculate_manifest_revision.rs` includes locale JSON in revisions. Verify the package with its existing reader and the relevant requested package checks. Report the locale, affected source keys and fallback behavior. Do not edit loader UI catalogs for a mod-only translation.
