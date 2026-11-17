---
{
  "id": "field-targets",
  "locale": "de",
  "title": "Feld `targets`",
  "summary": "Wählt den Client-Prozess, den Dedicated Server oder beide als Ausführungsort des Mods.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Ziele"
    ]
  }
}
---

# Feld `targets`

Wählt den Client-Prozess, den Dedicated Server oder beide als Ausführungsort des Mods.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

- Listenelemente: `"client"`, `"server"`
- Mindesteinträge: `1`
- Einträge müssen eindeutig sein.
- Legt fest, in welchen Prozessen der Mod läuft. EML-Pakete ohne targets laufen standardmäßig auf Client und Server. ShroudForge-Pakete laufen standardmäßig auf dem Client.

## Beispiel

```json
{
  "targets": [
    "client"
  ]
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
