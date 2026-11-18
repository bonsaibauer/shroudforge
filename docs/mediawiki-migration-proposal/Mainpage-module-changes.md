# Main page publication notes

The live Wiki already has the `Mainpage` template, module, CSS, gadget, and community cards. `Main_Page` transcludes `{{Mainpage}}`. The rendered main page reads its editable content from `Module:Mainpage/settings.json`.

## Prepared settings

`Mainpage-settings.json` is the review copy for that live JSON page. It uses the current module's `featured`, `patches`, `hero`, `chips`, and `directory` fields. It retains the live 1.7.9 release card and current hero artwork, and uses the uploaded image already displayed by `Quickstart: Shroudforge` for the featured card. It does not add a separate event card or replace the Wiki's existing daily-content panel.

The directory links to existing Wiki pages and categories, the external Lua API reference, and working features on the main website. The Mod Settings editor at `https://shroudforge.net/tools/modding/mod-settings/` and versioned schemas at `https://shroudforge.net/schemas/` are linked directly. The Browse mods routes now lead to `Category:Mods` and `Mods currently included`. The English mod articles have been created and marked for Translate. No PO translations were imported. Do not add an editor page to the Wiki or restore the stale draft links `Category:mod-json`, `Category:Hotkeys`, `Category:Lua-API`, `Category:DLLs`, `Category:Singleplayer`, or `Category:Multiplayer`.

The settings file remains a local review copy of the live `Module:Mainpage/settings.json`. That module page is editor-owned, gitignored, and skipped by `deploy-to-wiki`. When updating it, preserve the module's live editor notes and any unrelated community sections, then compare the saved JSON with this review copy.

## Existing behavior to preserve

- Keep the current `{{Mainpage}}` transclusion and its responsive layout.
- Keep the general Wiki's event and on-this-day content. Those panels belong to the wider Enshrouded community Wiki, not the ShroudForge documentation migration.
- Keep the ShroudForge Discord, funding, and social links in `Module:Mainpage/Community`.
- Keep the patch card tied to the currently published stable release. Refresh it only after a new stable release is published.
- Do not change the imported module templates, CSS, or skin as part of this index update. The prepared navigation is entirely settings data.

## Links to add after article migration

After the corresponding articles exist, add these landing pages to the main-page directory:

| Reader task | Destination |
| --- | --- |
| Multiplayer server setup | Group the server guides in the main-page directory and categorize each article under `Category:Guide:Shroudforge`. |
| Browse mods | `Category:Mods`, with a separate `Mod:` article for each package. |
| Read manifests and runtime guides | `Category:Modding:Shroudforge`. |
| Find `extended.mod.json` fields | Existing `Category:Mod-extended-json`. |
| Troubleshoot a problem | Existing `Category:Problems:Shroudforge`. |

Do not add new categories for multiplayer, built-in mods, World Editor, runtime and compatibility, localization, mod manifests, or modding guides. The only new catalogue category in this plan is `Category:Mods`. Add its mod articles before exposing it as a homepage shortcut. Never add a Lua API category.
