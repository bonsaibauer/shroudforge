# Feld `groups[].actions[].style`

Darstellung des Aktionsknopfs, ohne die Funktion zu ändern.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Werte: `"primary"`, `"secondary"`, `"danger"`

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "actions": [
        {
          "id": "reset",
          "label": "Reset",
          "style": "secondary"
        }
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended-mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
