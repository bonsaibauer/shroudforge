---
{
  "id": "field-authors",
  "locale": "de",
  "title": "Feld `authors`",
  "summary": "Namen der Personen, die den Mod erstellt haben.",
  "navigation": {
    "path": [
      "mod.json",
      "Grundfelder"
    ]
  }
}
---

# Feld `authors`

Namen der Personen, die den Mod erstellt haben.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

- Einträge müssen eindeutig sein.

## Beispiel

```json
{
  "authors": [
    "Your Name"
  ]
}
```

## Quelle

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
