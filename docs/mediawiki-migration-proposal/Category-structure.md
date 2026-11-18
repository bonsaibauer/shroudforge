# ShroudForge Wiki information architecture

This plan keeps the live Wiki's broad categories and gives the homepage task-based routes into them. It does not create a deep category tree. Article titles and categories serve different purposes. Titles identify a page. Categories collect related pages.

## Use the existing guide categories

| Category | Status | Content |
| --- | --- | --- |
| `Category:Guide:Shroudforge` | Existing | Getting started, use of the loader, player setup, server setup, and multiplayer guides. |
| `Category:Modding:Shroudforge` | Existing | Mod creation, manifests, technical resources, compatibility, runtime, and localization guidance. |
| `Category:Mod-extended-json` | Existing | Detailed `extended.mod.json` settings references. Keep the current article assignment. |
| `Category:Problems:Shroudforge` | Existing | Symptoms, diagnostics, and recovery instructions. |
| `Category:Guide:Wiki` | Existing | How to write and translate Wiki articles. |
| `Category:Mods` | Use as the one mod catalogue | Each current or historical mod gets its own `Mod:` article and is placed directly in this category. The category now contains the individual mod articles and their English Translate source pages. No separate built-in-mod category is needed. |

Do not create extra categories for built-in mods, the World Editor, runtime and compatibility, localization and translation, multiplayer, manifests, or modding guides. These are article topics and homepage groupings. Put guides in `Guide:Shroudforge` or `Modding:Shroudforge`. Put complete mod entries in `Category:Mods`. Keep the existing `Mod-extended-json` category as the one technical sub-index already in use.

## Standalone mod articles

Each mod topic becomes a separate page. Every page uses a plain `Mod:` article title, describes one mod, and belongs to `Category:Mods`. The current repository package list also contains SF Crafting Essentials, which does not yet have a page in the GitHub Pages article set.

| Article title | Source topic |
| --- | --- |
| `Mod: Crafting Essentials` | SF Crafting Essentials, from `mods/sf-crafting-essentials/README.md` |
| `Mod: Auto Stamina Refill` | SF Auto Stamina Refill |
| `Mod: Infinite Item Split` | SF Infinite Item Split |
| `Mod: Infinite Item Use` | SF Infinite Item Use |
| `Mod: No Fall Damage` | SF No Fall Damage |
| `Mod: No Resource Cost` | SF No Resource Cost |
| `Mod: Production Time` | SF Production Time |
| `Mod: Survival` | SF Survival |
| `Mod: Unlimited Flight` | SF Unlimited Flight |
| `Mod: Unlock Blueprints` | SF Unlock Blueprints |
| `Mod: World Editor` | World Editor |

The current repository contains SF Crafting Essentials, SF Survival, SF Unlimited Flight, SF Unlock Blueprints, and World Editor. The remaining six source topics are historical packages. Check package manifests and release contents again when publishing because the Wiki listing must reflect the current release, not an old website summary.

`Mods currently included` remains a catalogue or overview article that links to those entries. It is not a replacement for them and is not a category. Put each mod's detailed setting documentation directly in that mod page as named sections or tables. Do not make a category or a separate page for each mod setting. Keep the six historical packages clearly marked as archived. The current World Editor package has its own standalone page and should not be labelled historical while it remains in the repository and release.

## Page titles and homepage routes

Keep the existing `Quickstart: Shroudforge` page title for compatibility with current links. Use direct task titles for new guides, for example `Install ShroudForge on Windows`, `Install ShroudForge on a dedicated server`, and `Set up a Docker server`. Multiplayer is a guide series, not a category. Those articles belong to `Guide:Shroudforge` and the homepage can group them under **Multiplayer**.

Use the main page to offer reader journeys instead of making categories do every job:

1. Start here: Quickstart and installation.
2. Play: player use and shortcuts.
3. Multiplayer: client/server differences and server setup.
4. Create a mod: Lua overview, manifest and settings guides.
5. Browse mods: the `Category:Mods` catalogue with a separate page for each mod.
6. Fix a problem: `Category:Problems:Shroudforge`.

The searchable Lua, game-type, and resource API reference stays on the existing GitHub Pages site. Link to that one reference from the homepage and the relevant Lua overview. Do not create a `Category:Lua API` or Wiki articles for each API function or symbol.

## How to classify an article

- General getting-started, usage, player, server, and multiplayer instructions belong in `Category:Guide:Shroudforge`.
- Mod authoring, manifest, runtime, compatibility, and translation guidance belongs in `Category:Modding:Shroudforge`.
- `extended.mod.json` field details keep `Category:Mod-extended-json` as their existing focused index. The overview can also be in `Category:Modding:Shroudforge`.
- Mod catalogue entries belong in `Category:Mods`. Each `Mod:` page represents one mod, including former built-in mods.
- Recovery instructions belong in `Category:Problems:Shroudforge`.
- Wiki contribution instructions belong in `Category:Guide:Wiki`.

Give each article one primary category. Add one additional category only when an existing cross-topic index makes it materially easier to find. Do not make a new category just to mirror a homepage card or a website menu label.

For multilingual articles, keep one English source and its translations in the current MediaWiki Translate group. Never hand-create `/de`, `/ja`, or other language subpages. Follow the conventions in `docs/mediawiki-translate/README.md`.

## Migration boundary

The GitHub Pages site currently indexes 177 bilingual pages across `creating-mods`, `using-shroudforge`, and `platform`. The older proposal's count of 172 is stale.

Migrate the in-scope instructional content at full depth. Convert interactive website views into readable Wiki articles and preserve their instructions, examples, tables, warnings, and links. Keep a separate external link when the Wiki cannot replace an actual editor or searchable tool.

The Lua API reference is explicitly out of scope. Do not create pages for the symbols or individual API calls. The page inventory and the consolidation plan for the independent mod articles are in `Content-migration-map.md`.
