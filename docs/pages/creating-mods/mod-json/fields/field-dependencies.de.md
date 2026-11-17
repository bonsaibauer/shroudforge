---
{
  "id": "field-dependencies",
  "locale": "de",
  "title": "Feld `dependencies`",
  "summary": "Andere Mods, deren Verfügbarkeit oder Version der Loader vor dem Start prüfen muss.",
  "navigation": {
    "path": [
      "mod.json",
      "Abhängigkeiten"
    ],
    "overview": true,
    "label": "Übersicht"
  }
}
---

# Feld `dependencies`

Andere Mods, deren Verfügbarkeit oder Version der Loader vor dem Start prüfen muss.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

- Mods, die vor dem Start dieses Mods verfügbar sein müssen. id muss der Mod-ID aus der anderen mod.json entsprechen. version ist ein SemVer-Bereich. Bei optional: true blockiert eine fehlende, deaktivierte oder unpassende Abhängigkeit den Start nicht.

## Beispiel

```json
{
  "dependencies": [
    {
      "id": "example",
      "version": "example"
    }
  ]
}
```

## Quelle

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
