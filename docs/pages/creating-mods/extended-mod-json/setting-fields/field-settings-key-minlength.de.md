---
{
  "id": "field-settings-key-minlength",
  "locale": "de",
  "title": "Feld `settings.<key>.minLength`",
  "summary": "Mindestlänge eines Textwerts.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Einstellungen",
      "Einstellungsfelder"
    ]
  }
}
---

# Feld `settings.<key>.minLength`

Mindestlänge eines Textwerts.

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
      "minLength": 3
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
