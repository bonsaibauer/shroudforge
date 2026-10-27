# Translate mod text

Translate visible Modloader text and Lua text from the same locale files in the mod folder. Read the Modloader language through `shroudforge.locale()`, added by PR #42.

## Locale files

Add one file under `locales/<code>.json` for each language. Current codes from the Modloader language selector are `de`, `de-AT`, `de-BAY`, `de-CH`, `en-GB`, `en-US`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv`, and `zh`. The filename matches its code exactly. These files sit beside `mod.json` and `extended.mod.json`.

The schema is published as [`mod-locale.schema.json`](https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json). A locale catalog can translate `mod.name` and `mod.description`, setting and option labels, groups, actions, changelog lines, and Lua `strings`. Catalogs may be partial. Entries without a translation keep their source text.

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json",
  "settings": {
    "severity": {
      "label": "Log level",
      "options": { "info": "Information", "warn": "Warning" }
    }
  },
  "groups": [
    { "label": "Logging", "actions": { "write": { "label": "Write message" } } }
  ],
  "strings": { "overlay.ready": "Ready" }
}
```

Setting and option keys stay the same as in `extended.mod.json`. Group translations follow that file's group order. Action translations use the existing action ID. Saved values do not change.

## Use the same text in Lua

```lua
local locale = shroudforge.locale()
local ready = shroudforge.i18n.get("overlay.ready", locale, "Ready")
```

`shroudforge.i18n.get(key, locale, fallback)` uses the same loaded catalog as the Modloader. It checks a regional code such as `de-AT` first, followed by its language `de`. A missing string returns the fallback text, or the key when no fallback is provided. The lookup does not read files again. PR #42 supplies the language API, and this feature connects it to each mod's catalog.

If `shroudforge.locale()` returns `en` because the configuration cannot be read, catalog lookup checks `en-GB.json` first and then `en.json`. `en-GB` is the Modloader's English default. `en-US` remains available as an explicitly selected variant.

Locale files do not require the runtime capability. ShroudForge interface text stays in the shared Modloader translations.
