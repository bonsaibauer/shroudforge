---
{
  "id": "modding-locales",
  "locale": "en",
  "title": "Localize mod text and settings",
  "summary": "Mod files under locales/<code>.json use the same language code as the Modloader setting and serve both the Modloader and Lua from one catalog.",
  "navigation": {
    "path": ["Modding"],
    "label": "Localize mod text"
  },
  "apiSymbols": ["shroudforge.locale", "shroudforge.i18n.get"]
}
---

# Localize mod text and settings

The Modloader stores the selected language in `general.locale`. PR #42 adds `shroudforge.locale()` for mods to read it. A mod uses that same code for its files under `locales/`. One loaded catalog translates the visible Modloader text and supplies mod-authored Lua text.

## Files in the mod folder

Add one JSON file for each language you support, using the locale code unchanged. Current codes from the Modloader language selector are `de`, `de-AT`, `de-BAY`, `de-CH`, `en-GB`, `en-US`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv`, and `zh`. The filename must exactly match its code, for example `de-AT.json`.

```text
mods/your-mod/
├── mod.json
├── extended.mod.json
├── locales/
│   ├── de.json
│   └── de-AT.json
└── src/mod.lua
```

The schema is published as [`mod-locale.schema.json`](https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json). Locale files are optional and do not change `mod.json`, `extended.mod.json`, or saved setting values.

## Visible Modloader text

Translate the name and description under `mod`, setting labels and descriptions under `settings`, groups under `groups`, action buttons by their existing action IDs, and changelog lines. For choice settings, `options` maps each stored value to its translated display label. The value itself stays stable.

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json",
  "mod": { "name": "My Mod", "description": "A localized description." },
  "settings": {
    "severity": {
      "label": "Log level",
      "options": { "info": "Information", "warn": "Warning" }
    }
  },
  "groups": [
    {
      "label": "Logging",
      "actions": { "write": { "label": "Write message" } }
    }
  ],
  "strings": { "overlay.ready": "Ready" }
}
```

`settings` uses keys from `extended.mod.json`. Option keys are the stored option values. `groups` follows the group order in `extended.mod.json`, because groups do not have their own identifiers there. Action translations use the existing action ID. Text omitted from a catalog keeps its source text from `mod.json` and `extended.mod.json`.

## Lua text from the same catalog

`shroudforge.locale()` returns the language selected in the Modloader. Call it again if the language setting changes. Pass that code to `shroudforge.i18n.get`. There is no second Lua catalog or separate mod language choice, because these same files already supply the Modloader text.

```lua
local locale = shroudforge.locale()
local ready = shroudforge.i18n.get("overlay.ready", locale, "Ready")
```

`shroudforge.i18n.get(key, locale, fallback)` checks the exact code first, then the language part of a regional code. For `de-AT`, it checks `de-AT.json` before `de.json`. A missing entry returns `fallback`, or the key when there is no fallback. Catalogs are read when the mod loads, so Lua lookup does not read files.

If `shroudforge.locale()` returns `en` because the configuration cannot be read, catalog lookup checks `en-GB.json` first and then `en.json`. `en-GB` is the Modloader's English default. `en-US` remains available as an explicitly selected variant.

The `runtime` capability is not needed for this localization lookup. A mod only needs it when it uses other Lua runtime features. ShroudForge interface text stays in the shared Modloader translations.
