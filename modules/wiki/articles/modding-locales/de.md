# Mod-Texte übersetzen

Übersetze sichtbare Modloader-Texte und Lua-Texte mit denselben Locale-Dateien im Mod-Ordner. Die Modloader-Sprache wird über `shroudforge.locale()` gelesen. PR #42 ergänzt diese Abfrage.

## Locale-Dateien

Lege pro Sprache eine Datei unter `locales/<code>.json` an. Aktuelle Codes aus der Modloader-Sprachauswahl sind `de`, `de-AT`, `de-BAY`, `de-CH`, `en-GB`, `en-US`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv` und `zh`. Der Dateiname entspricht exakt dem Code. Die Dateien sind zusätzlich zu `mod.json` und `extended.mod.json`.

Das Schema steht unter [`mod-locale.schema.json`](https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json). Ein Locale-Katalog kann `mod.name` und `mod.description`, Einstellungs- und Optionsbeschriftungen, Gruppen, Aktionen, Changelog-Zeilen und Lua-`strings` enthalten. Unvollständige Dateien sind erlaubt. Nicht übersetzte Einträge verwenden den Quelltext.

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json",
  "settings": {
    "severity": {
      "label": "Protokollstufe",
      "options": { "info": "Information", "warn": "Warnung" }
    }
  },
  "groups": [
    { "label": "Protokoll", "actions": { "write": { "label": "Nachricht schreiben" } } }
  ],
  "strings": { "overlay.ready": "Bereit" }
}
```

Settings- und Optionsschlüssel bleiben dieselben wie in `extended.mod.json`. Gruppenübersetzungen folgen der Gruppenreihenfolge aus dieser Datei. Aktionsübersetzungen verwenden die vorhandene Aktions-ID. Gespeicherte Werte ändern sich nicht.

## Dieselben Texte in Lua verwenden

```lua
local locale = shroudforge.locale()
local ready = shroudforge.i18n.get("overlay.ready", locale, "Ready")
```

`shroudforge.i18n.get(key, locale, fallback)` verwendet denselben geladenen Katalog wie der Modloader. Es prüft zuerst einen regionalen Code wie `de-AT` und danach dessen Sprache `de`. Fehlt ein Text, liefert die Funktion den Rückfalltext oder den Schlüssel. Die Abfrage greift nicht erneut auf Dateien zu. Das API-Konzept stammt aus PR #42 und wird hier um den mod-eigenen Katalog erweitert.

Wenn `shroudforge.locale()` bei einer nicht lesbaren Konfiguration `en` liefert, prüft der Katalog zuerst `en-GB.json` und dann `en.json`. `en-GB` ist der englische Standard des Modloaders. `en-US` bleibt als ausdrücklich auswählbare Variante verfügbar.

Die Locale-Dateien benötigen keine Runtime-Berechtigung. ShroudForge-Oberflächentexte bleiben in den zentralen Übersetzungen des Modloaders.
