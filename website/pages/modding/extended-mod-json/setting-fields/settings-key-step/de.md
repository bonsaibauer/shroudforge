# Feld `settings.<key>.step`

Schrittweite für Zahlen- und Reglerfelder.

## Datentyp und Pflicht

`number` · optional

## Erlaubte Werte und Grenzen

- größer als: `0`

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": 1,
      "control": "number",
      "step": 0.1
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
