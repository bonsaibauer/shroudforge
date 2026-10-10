# Feld `groups[].actions[].id`

Eindeutiger Aktionsname, den der Mod in Lua registriert.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- Muster: `^[A-Za-z0-9._-]{1,80}$`

## Beispiel

```json
{
  "groups": [
    {
      "label": "Example",
      "actions": [
        {
          "id": "resetFeature",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended-mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
