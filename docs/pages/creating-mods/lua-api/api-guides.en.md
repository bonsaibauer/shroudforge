---
{
  "id": "api-guides",
  "locale": "en",
  "title": "Lua API, guides by feature",
  "summary": "The API reference lists every available symbol. These guides explain common tasks with their flow, prerequisites, and complete examples.",
  "navigation": {
    "path": [
      "Lua API guides"
    ],
    "overview": true,
    "label": "Overview"
  }
}
---

# Lua API, guides by feature

The [searchable API reference](#api) is the complete index of implemented functions and types. These guides explain common tasks with context and working examples.

## Common tasks

- [Publish Modloader notices](#doc-api-notifications) explains `shroudforge.notifications.publish` and how notices appear in the Modloader.
- [Read mod settings](#doc-api-settings) connects `extended.mod.json` to `shroudforge.settings.get`.
- [Connect Modloader actions](#doc-api-ui-actions) connects an action ID to `shroudforge.ui.on_action`.
- [Write log messages](#doc-api-logging) explains levels and useful messages.
- [Files and exports](#doc-api-files) explains package files and export capabilities.
- [Runtime and game features](#doc-api-runtime) explains lifecycle, client/server targets, and native support.
- [Detect the main menu](#doc-modding-main-menu) explains `runtime.is_main_menu()` and unknown loading transitions.
- [Read the character name](#doc-modding-player-name) explains `runtime.player.name()`, fallback names, and the shared cache.

## What each API article covers

Each guide explains purpose, when to use it, required capabilities, arguments, and results. Use the API search for the remaining functions, it links back to the Lua source contract. API functions may be EML-specific or ShroudForge-specific.
