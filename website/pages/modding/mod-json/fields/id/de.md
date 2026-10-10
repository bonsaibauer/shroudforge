# Feld `id`

Stabile Kennung, auf die andere Mods verweisen können.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- Höchstlänge: `80`
- Muster: `^[A-Za-z0-9._-]+$`

## Beispiel

```json
{
  "id": "author.example-mod"
}
```

## Quelle

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
