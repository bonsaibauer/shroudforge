---
name: add-mod-link-provider
description: Add a supported mod detail link type with backend validation, display order, icon, appearance and localized labels. Use when introducing a new link provider, not when assigning an existing link key in one package.
---

# Add a Mod Link Provider

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Read `modules/modloader/mod_link_order.json`, `validate_mod_manifest.rs`, `ui/show-mod.tsx`, a neighboring `ui/links/*.json`, and `modules/ui-resources/show-header-link.tsx`.

Add the new stable link ID to the existing order list, which also serves backend acceptance. Add its display definition under `modules/modloader/ui/links/<id>.json` with `id`, `labelKey`, `descriptionKey`, `icon` and `appearance`. The frontend loads definitions through `import.meta.glob`, so do not add another registration list.

Use an existing icon when suitable. Existing provider SVG files live directly under `modules/modloader/ui/links/`. Inspect `modules/ui-resources/show-header-link.tsx` before adding an asset. Its current glob points at the absent `../modloader/ui/links/assets/*.svg` directory, so the checked-in layout and discovery do not currently match. Resolve asset discovery within the requested feature and verify the rendered icon, rather than copying this mismatch into another registry. Shared vector icons live in `modules/ui-resources/show-icon.tsx`. Add the requested appearance to the existing stylesheet and both text keys to the shared UI catalogs, preserving locale validation rules.

`extended.mod.json.links` maps this ID to an HTTPS URL. A package adopting an existing link key needs no provider change. Preserve `open_external_link.rs` URL handling and use the existing host command to open links.

Verify backend acceptance, order, definition discovery, label/icon fallback and relevant frontend/locale build checks. Update the package guide when the public link vocabulary changes. Report the supported ID and affected files.

The website playground maintains an independent `linkBadges` table in `docs/website/page-integrations/website-manifests/render-mod-manifest-editor.js`. Update this public authoring consumer and affected bilingual field documentation when the new provider becomes part of the documented vocabulary.
