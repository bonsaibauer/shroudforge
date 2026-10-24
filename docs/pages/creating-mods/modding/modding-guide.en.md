---
{
  "id": "modding-guide",
  "locale": "en",
  "title": "Modding with the Lua runtime",
  "summary": "Practical guides to the main-menu state and selected local character name provided by the shared runtime.",
  "navigation": {
    "path": ["Modding"],
    "overview": true,
    "label": "Overview"
  }
}
---

# Modding with the Lua runtime

These guides show how an active mod queries the main-menu state and selected local character name. Both come from the same native runtime used by the Modloader UI.

| Task | Lua function | Guide |
| --- | --- | --- |
| Detect the main menu | `runtime.is_main_menu()` | [Query the main-menu state](#doc-modding-main-menu) |
| Read the selected character name | `runtime.player.name()` | [Query the local character name](#doc-modding-player-name) |

The mod needs the [`runtime` capability](#doc-capability-runtime) and an active runtime callback. A runtime callback can already run in the main menu. `runtime.phase == "ingame"` describes the started mod runtime and does not confirm entry into a world.

The articles include prerequisites, return values, and complete Lua examples. More functions are documented in the [API reference](#api) and [Lua API guides](#doc-api-guides).
