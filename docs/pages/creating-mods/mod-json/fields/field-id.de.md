---
{
  "id": "field-id",
  "locale": "de",
  "title": "Feld `id`",
  "summary": "Stabile Kennung, auf die andere Mods verweisen können.",
  "navigation": {
    "path": [
      "mod.json",
      "Grundfelder"
    ]
  }
}
---

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

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
