---
{
  "id": "field-settings-key-maxlength",
  "locale": "de",
  "title": "Feld `settings.<key>.maxLength`",
  "summary": "Höchstlänge eines Textwerts.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Einstellungen",
      "Einstellungsfelder"
    ]
  }
}
---

# Feld `settings.<key>.maxLength`

Höchstlänge eines Textwerts.

## Datentyp und Pflicht

`integer` · optional

## Erlaubte Werte und Grenzen

- Minimum: `0`

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": "hello",
      "control": "text",
      "maxLength": 5
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
