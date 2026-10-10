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

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
