# Einstellung `general.locale`

Sprache der Modloader-Oberfläche, als gültiger Sprach-Tag.

## Wirkung

Sprache der Modloader-Oberfläche, als gültiger Sprach-Tag.

## Standardwert

`"de"`





## Datentyp und zulässige Werte

Typ: `string`.

- Muster: `^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$`

## Beispiel

Dies ist ein Teilausschnitt der Konfiguration. Andere Einträge bleiben bestehen.

```json
{
  "general": {
    "locale": "de"
  }
}
```

## Ort in der Konfiguration

`shroudforge/config/modloader-config.json`, JSON-Pfad `/general/locale`. Diese Einstellung konfiguriert den Loader und wird nicht in das Mod-Paket kopiert.

Quelle: `src/loader/package/src/config/loader.schema.json` und `loader.default.json`.
