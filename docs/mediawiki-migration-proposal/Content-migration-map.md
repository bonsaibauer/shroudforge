# GitHub Pages to MediaWiki migration map

This archived map was based on the 177 paired English and German pages that used to live under `docs/pages/`: 151 under `creating-mods`, 24 under `using-shroudforge`, and 2 under `platform`. The old page text and renderer have since been removed. The preserved article IDs and their destinations are in [`githubpages/redirect-routes.json`](../../githubpages/redirect-routes.json).

## Destination model

The homepage is organized by reader task. Category pages are the full indexes for a topic. Existing Wiki pages should be updated or linked when they already cover the same subject. Do not create a second article just to mirror a page title from GitHub Pages.

| GitHub Pages collection | Source count | Wiki destination | Treatment |
| --- | ---: | --- | --- |
| `creating-mods/mod-json/` | 18 | `Modding:Shroudforge` | Keep the overview and all field, dependency, capability, and schema explanations. |
| `creating-mods/extended-mod-json/` | 64 | `Modding:Shroudforge` and the existing `Category:Mod-extended-json` | Keep the full settings, groups, actions, controls, links, targets, presets, and field-reference coverage. |
| `creating-mods/built-in-mods/` | 50 | `Category:Mods` | Make one standalone `Mod:` page for each of the 10 mod topics. Keep `Mods currently included` as a catalogue linking to them. Integrate the 39 setting-reference sources into the matching mod pages as full sections. Do not create `Category:Built-in-mods`, `Category:World Editor`, or separate setting pages. Mark historical and current support accurately from the current package files. |
| `creating-mods/modding/` | 4 | `Modding:Shroudforge` | Keep the Lua modding overview and localization guide. Do not create standalone pages for the two individual API examples `main-menu-state` and `selected-player-name`. |
| `creating-mods/eml-compatibility/` | 1 | `Modding:Shroudforge` | Keep the package-format boundaries, migration steps, and compatibility limits without creating a runtime category. |
| `creating-mods/lua-api/` | 11 | Outside the Wiki | Keep the searchable Lua, native, type, and resource reference material on the existing GitHub Pages site. Do not produce per-feature or per-symbol Wiki articles from this folder. Link to the reference as one external resource. |
| `creating-mods/` interactive views | 2 in scope | `Guide:Shroudforge` and `Modding:Shroudforge` | Convert `website-first` into a readable tutorial. Turn the written `website-manifests` guidance into a complete Wiki article with its examples and explanations. The live Mod Settings editor already exists at `https://shroudforge.net/tools/modding/mod-settings/`, and the schema catalogue already exists at `https://shroudforge.net/schemas/`. Link to these maintained website features from the article. Do not rebuild either feature in the Wiki. |
| `creating-mods/website-api` | 1 | Outside the Wiki | This is the searchable API catalogue view. Leave it with the API reference and do not create a Wiki landing page that duplicates its data. |
| `using-shroudforge/` player and Quickstart views | 5 | `Guide:Shroudforge` | Convert the PC setup, shared server files, server overview, Windows server, and Docker server views into step-by-step Wiki instructions. Group the multiplayer articles together on the main page without adding a multiplayer category. Preserve path examples, commands, prerequisites, environment variables, caveats, and links. |
| `using-shroudforge/modloader-settings/` | 17 | `Guide:Shroudforge` | Keep the settings overview and all individual preference details. Organize the entries into a readable settings guide and linked reference pages where useful. Do not collapse away defaults, accepted values, paths, or restart behavior. |
| `using-shroudforge/website-home` | 1 | Wiki main page and `Guide:Shroudforge` | Use the concise orientation material in the homepage introduction and retain any distinct explanatory content in the getting-started article. Do not create a duplicate homepage article. |
| `using-shroudforge/website-community` | 1 | Wiki community section | Keep useful community and support destinations in the homepage community card. Do not create an empty directory article that only repeats those links. |
| `platform/` | 2 | `Modding:Shroudforge` | Keep the runtime architecture and compatibility explanations as guides, not as Lua symbol references. Preserve their limits and source links without creating a runtime category. |

The current view count is 12: one homepage view, one community view, three modding views, five Quickstart/player/server views, and two platform views. The searchable API catalogue and the homepage view remain outside the article migration. The community card content belongs on the Wiki homepage. The existing website editor and schema catalogue remain on `shroudforge.net` and are linked from the relevant Wiki articles. All in-scope instructional views need readable article equivalents.

The 11 pages under `creating-mods/lua-api/`, the `website-api` catalogue, and the two individual API example pages do not become Wiki articles. That leaves 163 website source pages in scope. The 39 mod-setting sources become full sections inside the matching `Mod:` articles. The homepage and community source views feed the Wiki's main page instead of becoming duplicate articles. This leaves 122 standalone article destinations from GitHub Pages plus the integrated mod-setting sections. Add `Mod: Crafting Essentials` from the current repository package README so the live mod catalogue covers all five repository packages. The prepared catalogue therefore has 123 standalone article destinations after that additional package page.

## Existing in-product help articles

These 13 articles already have a local MediaWiki source and a publication entry. Keep them as the canonical articles for those subjects and connect them to the existing categories. Update their text only when the website migration supplies a more complete, current explanation.

| Existing article | Current or proposed category | Migration action |
| --- | --- | --- |
| Dependencies | `Modding:Shroudforge` | Keep the translated source and connect it from the manifest guide. |
| DInput8 chain loading standard | `Modding:Shroudforge` | Keep the shared compatibility specification and its existing CC0 attribution. |
| DLL Manager | `Modding:Shroudforge` | Keep the current step-by-step help. |
| Engine | `Modding:Shroudforge` | Keep the engine terminology reference. |
| Automatic beta game profiles | `Guide:Shroudforge` | Keep the automatic profile-discovery guide. |
| Keyboard shortcuts | `Guide:Shroudforge` | Keep the in-game shortcut and remapping article. |
| Mod Detection | `Problems:Shroudforge` | Keep the troubleshooting instructions. |
| Search and organize large mod settings | `Modding:Shroudforge` and `Mod-extended-json` | Keep the current settings-search explanation and its extension category. |
| Modding | `Modding:Shroudforge` | Keep as the main Lua modding overview. |
| Translate mod text | `Modding:Shroudforge` | Keep the complete translation workflow. |
| Modloader P2P connection | `Modding:Shroudforge` | Keep the multiplayer transport, permission, and trust-boundary explanation. |
| Startup stops because asset backups are incompatible | `Problems:Shroudforge` | Keep the recovery procedure. |
| Translate the Modloader | `Guide:Shroudforge` | Keep the loader UI translation workflow. |

The 13 existing help sources are the baseline to preserve before adding migrated website articles. Do not replace their translated source with shorter migration summaries.

## Preserve content depth

For every migrated page, compare the English and German sources before conversion. Keep the full explanation rather than replacing it with a short card summary. Retain:

- all steps, prerequisites, defaults, limits, examples, commands, and recovery paths
- warnings that distinguish clients from dedicated servers, and game files from runtime changes
- complete JSON and Lua examples with the existing code intact
- tables of field names, accepted values, and behavior
- original explanatory links, replacing only links whose target is no longer published
- illustrations that teach a process, with their captions and accessible descriptions

Homepage summaries and category descriptions are short navigation aids. They are not substitutes for the article text. If a website view includes an editor, search widget, or other behavior that cannot run as Wiki text, preserve its written instructions in the Wiki and link to the existing maintained feature separately. In particular, reuse the Mod Settings editor and versioned schemas already published on `shroudforge.net`.

## Migration order

1. Finalize the category landing pages and the main-page directory. Use only existing article titles in quick links.
2. Convert the player Quickstart and the four multiplayer views. Update the current Quickstart so its server link points to the Wiki.
3. Convert the modding overview, first-mod tutorial, manifest guide, and EML compatibility guide.
4. Migrate all `mod.json` and `extended.mod.json` field references, keeping the overview pages easy to find.
5. Create the 10 standalone `Mod:` articles from the website collection, add `Mod: Crafting Essentials` from its current repository package, and create the single `Category:Mods` catalogue. Move the 39 setting descriptions into the matching mod pages. Mark six archived package pages clearly and verify current package status against `mods/README.md` and each manifest.
6. Migrate ShroudForge settings and platform/runtime articles into the existing `Guide:Shroudforge` and `Modding:Shroudforge` categories.
7. Compare all 13 existing in-product Wiki sources with their migrated counterparts, resolve overlap, and keep one canonical article per subject.
8. Verify every destination title, category, translation group, internal link, download/tool link, image, and code sample. Remove GitHub Pages links from migrated article paths except the external Lua API reference.

## Source of truth and publication

The legacy Markdown page source has been removed. The current Wiki sources live under `modules/wiki/articles/`, and the established publishing/translation workflow is documented in `docs/mediawiki-translate/README.md`. The root `githubpages/` folder preserves old URLs and links them to their current destinations.

Before publication, generate and inspect the Wiki sources and links. Publish in small related batches so each category and homepage link points to real pages. For each translated article, keep its English source and translations in the same MediaWiki Translate group.
