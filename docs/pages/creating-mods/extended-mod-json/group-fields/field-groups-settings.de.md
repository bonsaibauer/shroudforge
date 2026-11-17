---
{
  "id": "field-groups-settings",
  "locale": "de",
  "title": "Feld `groups[].settings`",
  "summary": "Schlüssel vorhandener Mod-Einstellungen, die in dieser Gruppe angezeigt werden.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Gruppen und Aktionen",
      "Gruppenfelder"
    ]
  }
}
---

# Feld `groups[].settings`

Schlüssel vorhandener Mod-Einstellungen, die in dieser Gruppe angezeigt werden.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

- Einträge müssen eindeutig sein.

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [
        "enabledFeature"
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
