# Category structure for the ShroudForge Wiki

The homepage keeps the imported Star Citizen main page layout, but its navigation is about creating and understanding Enshrouded mods.

## Proposed hierarchy

- Category:ShroudForge documentation
  - Category:Modding
    - Category:Modding guides
    - Category:Mod manifests
    - Category:Mod settings
    - Category:Lua API
    - Category:Built-in mods
      - Category:World Editor
    - Category:Runtime and compatibility
    - Category:Localization and translation
  - Category:Troubleshooting
  - Category:Using ShroudForge

The supplied categories folder contains the short English descriptions and parent-category tags. Add the matching category tag to each article at the bottom.

## Homepage navigation

The quick links under the search box point to the first mod tutorial, the two manifest formats, the Lua API guides, current bundled mods, and the World Editor archive. The bottom directory groups the content by what a mod author is trying to do.

## Where the source content belongs

| Source | Approximate scope | Wiki destination |
| --- | ---: | --- |
| docs/pages/creating-mods/website-first.en.md | First-mod tutorial | Modding guides |
| docs/pages/creating-mods/website-manifests.en.md | Manifest editor and settings overview | Mod manifests and Mod settings |
| docs/pages/creating-mods/mod-json/ | 18 pages | Mod manifests |
| docs/pages/creating-mods/extended-mod-json/ | 60 pages | Mod settings |
| docs/pages/creating-mods/lua-api/ | 11 pages | Lua API |
| docs/pages/creating-mods/built-in-mods/ | 50 pages | Built-in mods, with historical World Editor pages under World Editor |
| docs/pages/creating-mods/eml-compatibility/ | 1 page | Runtime and compatibility |
| Other creating-mods guides and overviews | 4 pages | Modding guides |
| docs/pages/platform/ | 2 pages | Runtime and compatibility |
| docs/pages/using-shroudforge/ | 24 pages | Using ShroudForge |
| modules/wiki/articles/ | 10 in-product help articles | Distribute among manifests, runtime, troubleshooting, modding guides, and localization |

The docs website contains 172 English pages across its three sections. The in-product Wiki is a separate source with 10 additional English help articles.

## Migration order

1. Upload the four PNG assets and create the category pages.
2. Add the homepage articles in articles/. Then paste Mainpage-settings.json into Module:Mainpage/settings.json.
3. Migrate the remaining mod.json, extended.mod.json, and Lua API reference pages in batches. Keep each source article's title and technical details.
4. Add built-in mod pages and clearly mark archived packages such as World Editor.
5. Add runtime, compatibility, troubleshooting, localization, and player guides.
6. Check all internal links and replace any wiki-specific links that do not exist on ShroudForge.

Do not import the source wiki's Main_Page or Gadget definitions as part of this content migration.
