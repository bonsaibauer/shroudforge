# ShroudForge documentation website

The website is a static, bilingual player guide, server guide, modding tutorial,
visual manifest builder and live preview, and generated API/game-data reference.
Modding reference articles are Markdown pages stored in their own folders.

## Routes and content

- `index.html` selects German or English from the saved preference/browser locale.
- `de/index.html` and `en/index.html` are directly shareable language entry points.
- `content.de.js` and `content.en.js` contain the translated guide copy, examples, step-by-step lessons, and flow diagrams.
- `server-guides.de.js` and `server-guides.en.js` define the Quickstart navigation and the separate multiplayer server guides.
- `platform-guides.de.js` and `platform-guides.en.js` explain the difference between a mod and ShroudForge, then show how the Lua runtime and KFC native runtime work together.
- `support-guides.de.js` and `support-guides.en.js` provide mod discovery links and direct links to the matching GitHub issue forms.
- `app.js` connects the visual manifest builder, local JSON checks, Modloader-style preview, navigation, and API catalog search.
- `pages/` contains bilingual Markdown articles. Each article folder has a `page.json`, `de.md`, and `en.md`. Keep article-only examples and images in that same folder.
- `pages/index.js` is generated from article metadata by `website/tools/build-pages.mjs`. Do not edit it by hand.
- `styles.css` contains the responsive visual system.
- `media/` contains the ShroudForge mark and the Modloader screenshot. The loading, file-role, runtime, and setting flows are drawn with responsive HTML and CSS so they stay readable on phones.
- `data/api.json` is generated from `src/loader/api/src/`.
- `data/current.json` selects the active Enshrouded snapshot in `data/`.

## Source of truth

Keep technical descriptions aligned with repository-owned contracts and code:

- `src/loader/package/src/registry/manifest.schema.json`
- `src/loader/package/src/registry/extended.mod.schema.json`
- `src/loader/workflow/`
- `src/loader/api/src/eml/v1/` and `src/loader/api/src/shroudforge/v1/`
- `src/loader/modules/modloader-ui/ui/src/`
- `docs/sf/Architecture.md`, `docs/sf/runtime.md`, `docs/sf/mod-packages.md`, the runtime profile guide, and the root `README.md`

The manifest studio runs in the browser on the user's device. It starts with
the `templates/mod/` example. It offers visual fields for manifest details,
client and server targets, setting controls, groups, action buttons, links, and changelog notes, plus two
always-visible JSON editors with separate copy/download controls and a combined
Modloader-style preview. Form fields and direct edits stay synchronized. The
English and German pages show how a declared setting connects to Lua. The
builder catches JSON syntax errors and common contract mistakes. It is not a
complete JSON Schema validator. The repository schemas and loader validation
define the full contract. A runtime Lua example also needs the `runtime`
capability in `mod.json`.

## Add a documentation page

Create a folder under `pages/` and add a `page.json`, `de.md`, and `en.md`.
Set a stable page `id`, localized `title`, `summary`, and `navGroup` values, then
choose `navOrder` and article `order`. The build discovers the folder and
regenerates the navigation index. The application renders Markdown in the
existing site layout. Use links such as `[Targets](#doc-targets)` to open another
article and keep examples or assets beside the page that uses them.

Schema field articles, Modloader preference articles, and built-in mod setting
articles are individual Markdown pages. Their initial content was seeded from
the checked-in schemas, defaults, and mod packages by
`website/tools/seed-reference-pages.mjs`. The Markdown files are the
reader-facing source. Edit the relevant page when a setting or its behavior
changes, then regenerate the navigation index with `npm run site:build`.

The seed tool creates missing pages by default. Pass `--overwrite` only when
you intentionally want to replace all seeded page content with the current
schema and package data. Review generated copy after doing that because
handwritten explanations may need to be restored.

## Build and publication

Run the website pipeline from the repository root with `npm run site:check`.
It rebuilds the API catalog and Markdown navigation index, sanitizes snapshot
data, generates runtime ECS reference data, and validates generated content. GitHub Pages publishes the
`website/` directory, so site assets and both language routes must live inside
this directory.

Do not edit generated snapshot outputs by hand. Update their source or
generator and rebuild them instead.
