---
{
  "id": "field-launcher",
  "locale": "de",
  "title": "Feld `launcher`",
  "summary": "Herkunft des Mod-Pakets für die Modloader-Anzeige.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Felder"
    ]
  }
}
---

# Feld `launcher`

Herkunft des Mod-Pakets für die Modloader-Anzeige.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Werte: `"EML"`, `"SF"`
- Herkunft des Mods. Für ShroudForge-Mods weglassen. EML eintragen, um die Herkunft eines migrierten EML-Mods zu erhalten.

## Beispiel

```json
{
  "launcher": "EML"
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
