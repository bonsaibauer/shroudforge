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

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
