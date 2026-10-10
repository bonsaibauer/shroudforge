# Feld `settings.<key>.description`

Kurzer Text für die Modseite.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Höchstlänge: `500`

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": true,
      "description": "example"
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
