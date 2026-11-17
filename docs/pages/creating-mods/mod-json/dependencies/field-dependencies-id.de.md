---
{
  "id": "field-dependencies-id",
  "locale": "de",
  "title": "Feld `dependencies[].id`",
  "summary": "Exakte Mod-ID aus der mod.json des benötigten Mods.",
  "navigation": {
    "path": [
      "mod.json",
      "Abhängigkeiten",
      "Abhängigkeitsfelder"
    ]
  }
}
---

# Feld `dependencies[].id`

Exakte Mod-ID aus der mod.json des benötigten Mods.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- Höchstlänge: `80`
- Muster: `^[A-Za-z0-9._-]+$`
- Exakte ID aus der mod.json des benötigten Mods.

## Beispiel

```json
{
  "dependencies": [
    {
      "id": "author.shared-library",
      "version": "^1.0.0"
    }
  ]
}
```

## Quelle

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
