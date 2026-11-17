---
{
  "id": "field-groups-label",
  "locale": "de",
  "title": "Feld `groups[].label`",
  "summary": "Spielerfreundlicher Name, der im Modloader angezeigt wird.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Gruppen und Aktionen",
      "Gruppenfelder"
    ]
  }
}
---

# Feld `groups[].label`

Spielerfreundlicher Name, der im Modloader angezeigt wird.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- Mindestlänge: `1`
- Höchstlänge: `120`

## Beispiel

```json
{
  "groups": [
    {
      "label": "Gameplay",
      "settings": []
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
