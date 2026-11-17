---
{
  "id": "field-groups-actions",
  "locale": "de",
  "title": "Feld `groups[].actions`",
  "summary": "Aktionsknöpfe, deren IDs Lua-Callbacks aufrufen.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Gruppen und Aktionen",
      "Gruppenfelder"
    ]
  }
}
---

# Feld `groups[].actions`

Aktionsknöpfe, deren IDs Lua-Callbacks aufrufen.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

Keine zusätzlichen Werte oder Grenzen im Schema angegeben.

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [],
      "actions": [
        {
          "id": "reset",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
