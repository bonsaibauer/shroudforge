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

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
