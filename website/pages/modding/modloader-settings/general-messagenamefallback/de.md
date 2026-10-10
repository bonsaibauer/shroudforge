# Einstellung `general.messageNameFallback`

Name, den Nachrichten verwenden, wenn die automatische Erkennung keinen eindeutigen Spielernamen findet.

## Wirkung

Name, den Nachrichten verwenden, wenn die automatische Erkennung keinen eindeutigen Spielernamen findet.

## Standardwert

`"Flameborn"`





## Datentyp und zulässige Werte

Typ: `string`.

- Mindestlänge: `1`
- Höchstlänge: `40`

## Beispiel

Dies ist ein Teilausschnitt der Konfiguration. Andere Einträge bleiben bestehen.

```json
{
  "general": {
    "messageNameFallback": "Flameborn"
  }
}
```

## Ort in der Konfiguration

`shroudforge/config/modloader-config.json`, JSON-Pfad `/general/messageNameFallback`. Diese Einstellung konfiguriert den Loader und wird nicht in das Mod-Paket kopiert.

Quelle: `src/loader/package/src/config/loader.schema.json` und `loader.default.json`.
