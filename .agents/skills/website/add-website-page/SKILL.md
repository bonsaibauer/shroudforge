---
name: add-website-page
description: Add or update a bilingual published ShroudForge website article under docs/pages/, with paired document metadata, automatic menu placement, and website build validation. Use for published pages rather than offline Modloader Wiki articles.
---

# Add a Published Website Page

Read `.agents/skills/repository/repository-working-rules.md`, `docs/website/add-and-order-website-pages.en.md`, `docs/website/README.md`, and the neighboring article pair.

Create `docs/pages/<topic>/<descriptive-name>.en.md` and `.de.md` with matching folders, filename stems, and page ID. Preserve existing article content before moving files.

Use JSON document headers between `---` delimiters. Required fields are `id`, `locale`, `title`, and `summary`. Ordinary articles also require `navigation.path`. Keep IDs lowercase and hyphenated. Localize titles, summaries, menu paths/labels, and article bodies. Shared metadata such as `view`, `source`, `apiSymbols`, `apiPrefixes`, and overview settings must remain consistent between locales.

`docs/website/tools/read-doc-pages.mjs` discovers pairs and validates metadata. Relative folders and filenames determine ordering. Do not create a second route list, ordering field, `page.json` collection, or copied article tree.

Ordinary articles use Markdown. `docs/website/markdown-rendering.js` escapes raw HTML. Use `view` only for an intentional complete view whose paired Markdown bodies contain its HTML. Generated ordinary article routes use `#doc-<page-id>`. Complete views use the view name. Resolve media links relative to the source article.

`docs/website/tools/build-website-pages.mjs` generates the page index and view information. Page-specific interactivity belongs under `docs/website/page-integrations/<page-id>/` and uses its own integration workflow. Offline installed help remains under `modules/wiki/articles/`.

Run `npm run site:check` from the root or `./dev/build/check_website.ps1`. Inspect the generated page entry and article output under `build/website/` without editing generated files. For requested display verification, inspect both locales, route, menu placement, headings, code/media, and links.

Report source paths, page ID, routes, translations, navigation placement, and actual build/display checks.
