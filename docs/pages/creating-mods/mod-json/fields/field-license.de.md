---
{
  "id": "field-license",
  "locale": "de",
  "title": "Feld `license`",
  "summary": "Lizenzkennung, falls der Mod eine Lizenz erklärt.",
  "navigation": {
    "path": [
      "mod.json",
      "Grundfelder"
    ]
  }
}
---

# Feld `license`

Lizenzkennung, falls der Mod eine Lizenz erklärt.

## Datentyp und Pflicht

`string | null` · optional

## Erlaubte Werte und Grenzen

- Optionale Lizenzkennung, zum Beispiel MIT. Verwende null, wenn keine Lizenz angegeben wird.

## Beispiel

```json
{
  "license": "example"
}
```

## Quelle

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
