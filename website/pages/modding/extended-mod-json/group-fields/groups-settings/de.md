# Feld `groups[].settings`

Schlüssel vorhandener Mod-Einstellungen, die in dieser Gruppe angezeigt werden.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

- Einträge müssen eindeutig sein.

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [
        "enabledFeature"
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
