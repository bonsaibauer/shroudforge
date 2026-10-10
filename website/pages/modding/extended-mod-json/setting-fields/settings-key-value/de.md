# Feld `settings.<key>.value`

Startwert und erwarteter Lua-Datentyp der Einstellung.

## Datentyp und Pflicht

`boolean | string | number | array` · erforderlich

## Erlaubte Werte und Grenzen

Keine zusätzlichen Werte oder Grenzen im Schema angegeben.

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": true
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
