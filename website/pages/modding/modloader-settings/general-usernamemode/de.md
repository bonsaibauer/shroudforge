# Einstellung `general.usernameMode`

Legt fest, ob der Spielername automatisch erkannt oder manuell festgelegt wird.

## Wirkung

Legt fest, ob der Spielername automatisch erkannt oder manuell festgelegt wird.

## Standardwert

`"auto"`





## Datentyp und zulässige Werte

Typ: `string`.

- Werte: `"auto"`, `"manual"`

## Beispiel

Dies ist ein Teilausschnitt der Konfiguration. Andere Einträge bleiben bestehen.

```json
{
  "general": {
    "usernameMode": "auto"
  }
}
```

## Ort in der Konfiguration

`shroudforge/config/modloader-config.json`, JSON-Pfad `/general/usernameMode`. Diese Einstellung konfiguriert den Loader und wird nicht in das Mod-Paket kopiert.

Quelle: `src/loader/package/src/config/loader.schema.json` und `loader.default.json`.
