# Feld `groups[].description`

Kurzer Text für die Modseite.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Höchstlänge: `500`

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [],
      "description": "example"
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
