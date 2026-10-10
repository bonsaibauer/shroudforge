# Einstellung `paths.updates`

Ordner für heruntergeladene oder vorbereitete Updates.

## Wirkung

Ordner für heruntergeladene oder vorbereitete Updates.

## Standardwert

Automatisch aus dem Spielordner ermittelt

Ein nicht gesetzter Wert wird beim Start relativ zum Spielordner aufgelöst. Ein relativer eigener Pfad bezieht sich ebenfalls auf den Spielordner und wird danach als absoluter Pfad gespeichert. Ein absoluter Pfad bleibt erhalten.



## Datentyp und zulässige Werte

Typ: `string | null`.

- Mindestlänge: `1`

## Beispiel

Dies ist ein Teilausschnitt der Konfiguration. Andere Einträge bleiben bestehen.

```json
{
  "paths": {
    "updates": "./custom-updates"
  }
}
```

## Ort in der Konfiguration

`shroudforge/config/modloader-config.json`, JSON-Pfad `/paths/updates`. Diese Einstellung konfiguriert den Loader und wird nicht in das Mod-Paket kopiert.

Quelle: `src/loader/package/src/config/loader.schema.json` und `loader.default.json`.
