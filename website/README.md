# ShroudForge documentation website

The website is a static, bilingual player guide, server guide, modding tutorial,
visual manifest builder and live preview, and generated API/game-data reference.

## Routes and content

- `index.html` selects German or English from the saved preference/browser locale.
- `de/index.html` and `en/index.html` are directly shareable language entry points.
- `content.de.js` and `content.en.js` contain the translated guide copy, examples, step-by-step lessons, and flow diagrams.
- `app.js` connects the visual manifest builder, local JSON checks, Modloader-style preview, navigation, and API catalog search.
- `styles.css` contains the responsive visual system.
- `media/` contains the ShroudForge mark and the Modloader screenshot. The loading, file-role, and setting flows are drawn with responsive HTML and CSS so they stay readable on phones.
- `data/api.json` is generated from `src/loader/api/src/`.
- `data/current.json` selects the active Enshrouded snapshot in `data/`.

## Source of truth

Keep technical descriptions aligned with repository-owned contracts and code:

- `src/loader/package/src/registry/manifest.schema.json`
- `src/loader/package/src/registry/extended.mod.schema.json`
- `src/loader/workflow/`
- `src/loader/api/src/eml/v1/` and `src/loader/api/src/shroudforge/v1/`
- `src/loader/modules/modloader-ui/ui/src/`
- `docs/sf/Architecture.md`, `docs/sf/mod-packages.md`, and the root `README.md`

The manifest studio runs in the browser on the user's device. It starts with
the `templates/mod/` example. It offers visual fields for manifest details,
setting controls, groups, action buttons, links, and changelog notes, plus a
direct JSON editor, copy/download controls, and a Modloader-style preview. The
English and German pages show how a declared setting connects to Lua. The
builder catches JSON syntax errors and common contract mistakes; it is not a
complete JSON Schema validator. The repository schemas and loader validation
define the full contract. A runtime Lua example also needs the `runtime`
capability in `mod.json`.

## Build and publication

Run the website pipeline from the repository root with `npm run site:check`.
It rebuilds the API catalog, sanitizes snapshot data, generates runtime ECS
reference data, and validates generated content. GitHub Pages publishes the
`website/` directory, so site assets and both language routes must live inside
this directory.

Do not edit generated snapshot outputs by hand. Update their source or
generator and rebuild them instead.
