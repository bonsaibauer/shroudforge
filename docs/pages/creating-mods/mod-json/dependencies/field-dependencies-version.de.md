---
{
  "id": "field-dependencies-version",
  "locale": "de",
  "title": "Feld `dependencies[].version`",
  "summary": "SemVer-Bereich, der für die Version des benötigten Mods akzeptiert wird.",
  "navigation": {
    "path": [
      "mod.json",
      "Abhängigkeiten",
      "Abhängigkeitsfelder"
    ]
  }
}
---

# Feld `dependencies[].version`

SemVer-Bereich, der für die Version des benötigten Mods akzeptiert wird.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- SemVer-Versionsbereich, zum Beispiel ^1.2.0 oder >=1.2.0, <2.0.0.

## Beispiel

```json
{
  "dependencies": [
    {
      "id": "helper.mod",
      "version": "^1.2.0"
    }
  ]
}
```

## Quelle

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
