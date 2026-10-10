# Einstellung `logging.minimumLevel`

Niedrigste Stufe, die ShroudForge in Logdateien speichert. Die Debug Console hat zusätzlich einen eigenen Anzeigefilter.

## Wirkung

Niedrigste Stufe, die ShroudForge in Logdateien speichert. Die Debug Console hat zusätzlich einen eigenen Anzeigefilter.

## Standardwert

`"INFO"`





## Datentyp und zulässige Werte

Typ: `string`.

- Werte: `"TRACE"`, `"DEBUG"`, `"INFO"`, `"WARN"`, `"ERROR"`

## Beispiel

Dies ist ein Teilausschnitt der Konfiguration. Andere Einträge bleiben bestehen.

```json
{
  "logging": {
    "minimumLevel": "INFO"
  }
}
```

## Ort in der Konfiguration

`shroudforge/config/modloader-config.json`, JSON-Pfad `/logging/minimumLevel`. Diese Einstellung konfiguriert den Loader und wird nicht in das Mod-Paket kopiert.

Quelle: `src/loader/package/src/config/loader.schema.json` und `loader.default.json`.
