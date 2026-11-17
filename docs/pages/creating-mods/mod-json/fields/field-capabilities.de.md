---
{
  "id": "field-capabilities",
  "locale": "de",
  "title": "Feld `capabilities`",
  "summary": "Zugriffsarten, die der Mod anfordert.",
  "navigation": {
    "path": [
      "mod.json",
      "Zugriffsrechte"
    ],
    "overview": true,
    "label": "Übersicht"
  }
}
---

# Feld `capabilities`

Zugriffsarten, die der Mod anfordert.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

- Listenelemente: `"patch"`, `"export"`, `"runtime"`, `"runtime-register-dll"`
- Einträge müssen eindeutig sein.

## Beispiel

```json
{
  "capabilities": [
    "runtime"
  ]
}
```

## Quelle

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
