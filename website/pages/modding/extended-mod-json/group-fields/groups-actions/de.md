# Feld `groups[].actions`

Aktionsknöpfe, deren IDs Lua-Callbacks aufrufen.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

Keine zusätzlichen Werte oder Grenzen im Schema angegeben.

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [],
      "actions": [
        {
          "id": "reset",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
