---
{
  "id": "field-groups-actions-id",
  "locale": "de",
  "title": "Feld `groups[].actions[].id`",
  "summary": "Eindeutiger Aktionsname, den der Mod in Lua registriert.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Gruppen und Aktionen",
      "Aktionsfelder"
    ]
  }
}
---

# Feld `groups[].actions[].id`

Eindeutiger Aktionsname, den der Mod in Lua registriert.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- Muster: `^[A-Za-z0-9._-]{1,80}$`

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "actions": [
        {
          "id": "resetFeature",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
