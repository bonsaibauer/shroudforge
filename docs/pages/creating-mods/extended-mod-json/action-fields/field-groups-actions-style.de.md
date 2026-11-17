---
{
  "id": "field-groups-actions-style",
  "locale": "de",
  "title": "Feld `groups[].actions[].style`",
  "summary": "Darstellung des Aktionsknopfs, ohne die Funktion zu ändern.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Gruppen und Aktionen",
      "Aktionsfelder"
    ]
  }
}
---

# Feld `groups[].actions[].style`

Darstellung des Aktionsknopfs, ohne die Funktion zu ändern.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Werte: `"primary"`, `"secondary"`, `"danger"`

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "actions": [
        {
          "id": "reset",
          "label": "Reset",
          "style": "secondary"
        }
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
