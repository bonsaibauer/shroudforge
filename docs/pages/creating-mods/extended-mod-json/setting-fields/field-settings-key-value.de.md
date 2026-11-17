---
{
  "id": "field-settings-key-value",
  "locale": "de",
  "title": "Feld `settings.<key>.value`",
  "summary": "Startwert und erwarteter Lua-Datentyp der Einstellung.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Einstellungen",
      "Einstellungsfelder"
    ]
  }
}
---

# Feld `settings.<key>.value`

Startwert und erwarteter Lua-Datentyp der Einstellung.

## Datentyp und Pflicht

`boolean | string | number | array` · erforderlich

## Erlaubte Werte und Grenzen

Keine zusätzlichen Werte oder Grenzen im Schema angegeben.

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": true
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
