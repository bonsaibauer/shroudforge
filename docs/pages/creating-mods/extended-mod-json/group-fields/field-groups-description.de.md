---
{
  "id": "field-groups-description",
  "locale": "de",
  "title": "Feld `groups[].description`",
  "summary": "Kurzer Text für die Modseite.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Gruppen und Aktionen",
      "Gruppenfelder"
    ]
  }
}
---

# Feld `groups[].description`

Kurzer Text für die Modseite.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Höchstlänge: `500`

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [],
      "description": "example"
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
