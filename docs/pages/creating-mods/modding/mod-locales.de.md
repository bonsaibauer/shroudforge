---
{
  "id": "modding-locales",
  "locale": "de",
  "title": "Mod-Texte und Einstellungen lokalisieren",
  "summary": "Mod-Dateien unter locales/<code>.json verwenden denselben Sprachcode wie die Modloader-Einstellung und versorgen Modloader sowie Lua aus einem gemeinsamen Katalog.",
  "navigation": {
    "path": ["Modding"],
    "label": "Mod-Texte lokalisieren"
  },
  "apiSymbols": ["shroudforge.locale", "shroudforge.i18n.get"]
}
---

# Mod-Texte und Einstellungen lokalisieren

Der Modloader stellt die gewählte Sprache über `general.locale` ein. PR #42 ergänzt dafür `shroudforge.locale()`. Ein Mod verwendet genau diesen Sprachcode für eigene Dateien unter `locales/`. Derselbe geladene Katalog übersetzt die sichtbaren Modloader-Texte und liefert Mod-Texte an Lua.

## Dateien im Mod-Ordner

Lege für jede unterstützte Sprache eine JSON-Datei mit dem unveränderten Locale-Code an. Die aktuellen Codes aus der Modloader-Sprachauswahl sind `de`, `de-AT`, `de-BAY`, `de-CH`, `en-GB`, `en-US`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv` und `zh`. Der Dateiname muss exakt dem Code entsprechen, zum Beispiel `de-AT.json`.

```text
mods/your-mod/
├── mod.json
├── extended.mod.json
├── locales/
│   ├── de.json
│   └── de-AT.json
└── src/mod.lua
```

Das Schema steht unter [`mod-locale.schema.json`](https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json). Locale-Dateien sind optional und ändern weder `mod.json` noch `extended.mod.json` oder die gespeicherten Einstellungswerte.

## Sichtbare Modloader-Texte

Übersetze den Namen und die Beschreibung unter `mod`, Einstellungsnamen und Beschreibungen unter `settings`, Gruppen in `groups`, Aktionsknöpfe über ihre vorhandene Aktions-ID und Changelog-Zeilen. Bei Auswahlfeldern ordnet `options` den gespeicherten Wert dem übersetzten Anzeigetext zu. Der Wert selbst bleibt stabil.

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json",
  "mod": { "name": "Mein Mod", "description": "Eine lokalisierte Beschreibung." },
  "settings": {
    "severity": {
      "label": "Protokollstufe",
      "options": { "info": "Information", "warn": "Warnung" }
    }
  },
  "groups": [
    {
      "label": "Protokoll",
      "actions": { "write": { "label": "Nachricht schreiben" } }
    }
  ],
  "strings": { "overlay.ready": "Bereit" }
}
```

`settings` verwendet die Schlüssel aus `extended.mod.json`. Optionsschlüssel sind die gespeicherten Optionswerte. `groups` folgt der Reihenfolge der Gruppen in `extended.mod.json`, da diese dort keine eigene Kennung haben. Aktionsübersetzungen verwenden die vorhandene Action-ID. Nicht angegebene Texte behalten den Quelltext aus `mod.json` und `extended.mod.json`.

## Lua-Texte aus demselben Katalog

`shroudforge.locale()` gibt die im Modloader gewählte Sprache zurück. Rufe die Funktion bei Bedarf erneut auf, wenn sich die Spracheinstellungen ändern. Übergib den Code an `shroudforge.i18n.get`. Es gibt keinen zweiten Lua-Katalog und keine separate Mod-Auswahl, weil dieselben Dateien bereits für die Modloader-Texte verwendet werden.

```lua
local locale = shroudforge.locale()
local ready = shroudforge.i18n.get("overlay.ready", locale, "Ready")
```

`shroudforge.i18n.get(key, locale, fallback)` prüft zuerst den exakten Code, danach den Sprachanteil einer regionalen Sprache. Für `de-AT` gilt also `de-AT.json` vor `de.json`. Fehlende Einträge verwenden `fallback`, andernfalls den Schlüssel. Die Kataloge werden beim Laden des Mods gelesen, die Lua-Abfrage liest keine Dateien.

Wenn `shroudforge.locale()` wegen einer nicht lesbaren Konfiguration `en` zurückgibt, sucht die Katalogauflösung zuerst `en-GB.json` und dann `en.json`. `en-GB` ist der englische Standard des Modloaders. `en-US` bleibt als ausdrücklich auswählbare Variante verfügbar.

Die `runtime`-Berechtigung ist für diese Übersetzungsabfrage nicht erforderlich. Die Mod benötigt sie nur, wenn sie weitere Lua-Runtime-Funktionen verwendet. ShroudForge-eigene Oberflächentexte bleiben in den zentralen Modloader-Übersetzungen.
