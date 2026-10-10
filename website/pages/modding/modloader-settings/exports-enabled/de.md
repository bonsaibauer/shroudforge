# Einstellung `exports.enabled`

Schaltet die Exportfunktionen für Mod-Ausgaben ein oder aus.

## Wirkung

Schaltet die Exportfunktionen für Mod-Ausgaben ein oder aus.

## Standardwert

`true`





## Datentyp und zulässige Werte

Typ: `boolean`.

Keine zusätzlichen Werte oder Grenzen im Schema angegeben.

## Beispiel

Dies ist ein Teilausschnitt der Konfiguration. Andere Einträge bleiben bestehen.

```json
{
  "exports": {
    "enabled": true
  }
}
```

## Ort in der Konfiguration

`shroudforge/config/modloader-config.json`, JSON-Pfad `/exports/enabled`. Diese Einstellung konfiguriert den Loader und wird nicht in das Mod-Paket kopiert.

Quelle: `src/loader/package/src/config/loader.schema.json` und `loader.default.json`.
