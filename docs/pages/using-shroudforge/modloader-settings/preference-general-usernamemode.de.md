---
{
  "id": "preference-general-usernamemode",
  "locale": "de",
  "title": "Einstellung `general.usernameMode`",
  "summary": "Legt fest, ob der Spielername automatisch erkannt oder manuell festgelegt wird.",
  "navigation": {
    "path": [
      "Modloader-Einstellungen",
      "Allgemeine Einstellungen"
    ]
  }
}
---

# Einstellung `general.usernameMode`

Legt fest, ob der Spielername automatisch erkannt oder manuell festgelegt wird.

## Wirkung

`"auto"` verwendet den gecachten Charakternamen aus der vorhandenen Runtime. Der native Menücallback liest ihn beim ersten gültigen Charakter und erneut bei geänderter Auswahl oder geändertem Namensdeskriptor. Während des Spielens bleibt der Name im Cache. Schlägt die Erfassung fehl, gilt `general.messageNameFallback`. Ein Name aus einem früheren Spielprozess wird nicht übernommen.

`"manual"` verwendet immer `general.messageNameFallback`. Dessen Standardwert ist `Flameborn`.

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

Quelle: `modules/loader-config/loader.schema.json` und `loader.default.json`.
