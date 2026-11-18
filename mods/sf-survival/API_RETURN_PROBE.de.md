# SF Survival API-Rückgabeprobe

Die Probe erfasst die acht gespeicherten SF-Survival-Einstellungen und die lesenden Runtime-Aufrufe im aktuellen Spielkontext. Die Ergebnisse stehen im ShroudForge-Protokoll und beginnen mit `[SF Survival API probe]`.

## Starten

1. Sichere `src/mod.lua` außerhalb des Mod-Pakets.
2. Ersetze in `src/mod.lua` vorübergehend den letzten `return { ... }`-Block durch:

   ```lua
   return require("api_return_probe")
   ```

3. Baue oder installiere SF Survival mit dieser temporären Änderung und lade den Mod.
4. Betrete eine lokale Welt. Die Probe wartet auf eine Welt-Session und erfasst die Rückgaben einmal.
5. Kopiere alle Protokollzeilen mit `[SF Survival API probe]`.
6. Stelle `src/mod.lua` aus der Sicherung wieder her und lade SF Survival erneut.

## Erfasste Aufrufe

Die Probe liest alle Settings, Runtime-Verfügbarkeit, Welt-Session, Spielerabfrage, Attribut-, Template- und Service-Ziele sowie die Rückgaben des Override-Trackers. Sie ruft `RuntimeOverrides:apply` nur mit dem gerade gelesenen Ausgangswert auf. Dadurch endet der Aufruf vor dem Schreibpfad und ändert keinen Spielwert.

`runtime.report_effect` veröffentlicht einmalig den Status `no-change` mit dem Hinweis `API return probe completed`. Das ändert keinen Spielwert. ShroudForge entfernt den temporären Status, wenn der Mod deaktiviert wird.

Opaque Runtime-Handles werden im Protokoll mit ihren lesbaren Identitätsfeldern ausgegeben. Ein nativer Handle selbst ist kein JSON-Wert und wird daher nicht serialisiert.
