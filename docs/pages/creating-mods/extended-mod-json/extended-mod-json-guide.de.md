---
{
  "id": "extended-mod-json-guide",
  "locale": "de",
  "title": "extended.mod.json, Einstellungen und Modloader-Seite",
  "summary": "Diese optionale Datei steuert Modloader-Einstellungen, Gruppen, Aktionen, Links, Zielprozesse und den gespeicherten Aktivierungszustand.",
  "navigation": {
    "path": [
      "extended.mod.json"
    ],
    "overview": true,
    "label": "Übersicht"
  }
}
---

# extended.mod.json

`extended.mod.json` ergänzt den Mod-Steckbrief. Sie wird verwendet, wenn dein Mod eigene Werte im Modloader anzeigen oder seinen Aktivierungszustand speichern soll. Jedes Feld hat eine eigene Seite in dieser Navigation.

## Das Wichtigste

- `schemaVersion` ist derzeit `1`.
- `enabled` speichert, ob der Mod eingeschaltet ist.
- `targets` legt fest, in welchem Prozess der Mod geladen wird. Das ist keine Netzwerk-Replikation.
- `settings` enthält Mod-eigene Werte.
- `groups` ordnet Werte in der Modloader-Oberfläche. Nicht gruppierte Einstellungen erscheinen ebenfalls, sie stehen unter einer Standardgruppe.
- `presets` lässt eine Auswahl wie Qualität mehrere Einstellungen auf einmal setzen. Siehe [Voreinstellungen](#doc-setting-presets).
- `launcher` kennzeichnet die Herkunft eines übernommenen EML-Mods.

## Beispiel

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json",
  "schemaVersion": 1,
  "enabled": false,
  "targets": ["client"],
  "settings": {
    "greeting": {
      "value": "Hello from my mod",
      "label": "Greeting",
      "control": "text"
    }
  }
}
```

Die Mod-Einstellung `greeting` wird in der Lua-API mit genau diesem Schlüssel gelesen. Siehe [Einstellungen und Steuerelemente](#doc-setting-controls).

## Quelle der Regeln

[extended.mod.schema.json](https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json) definiert Form und Datentypen. Der Paketleser prüft zusätzlich, ob Gruppen auf vorhandene Einstellungen zeigen und ob Auswahlwerte gültig sind.

[Mod-Einstellungen live ausprobieren](#manifests).
