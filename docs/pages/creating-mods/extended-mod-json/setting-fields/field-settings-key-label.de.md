---
{
  "id": "field-settings-key-label",
  "locale": "de",
  "title": "Feld `settings.<key>.label`",
  "summary": "Spielerfreundlicher Name, der im Modloader angezeigt wird.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Einstellungen",
      "Einstellungsfelder"
    ]
  }
}
---

# Feld `settings.<key>.label`

Spielerfreundlicher Name, der im Modloader angezeigt wird.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Mindestlänge: `1`
- Höchstlänge: `120`

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": true,
      "label": "example"
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
