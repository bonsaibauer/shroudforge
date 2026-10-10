# Modloader-Mitteilungen veröffentlichen

`shroudforge.notifications.publish` fügt eine mod-eigene Mitteilung in den lokalen Modloader-Feed ein. Sie wird nicht an andere Spieler gesendet. Der Mod muss die Funktion gezielt aufrufen, zum Beispiel nach einer wichtigen Änderung oder einem abgeschlossenen Schritt.

## Beispiel

```lua
shroudforge.notifications.publish({
  id = "world-ready",
  title = "World is ready",
  message = "Your world data has been checked.",
  level = "info"
})
```

Die konkrete Nutzlast und erlaubten Level stehen in der [API-Referenz](#api). Der Mod benötigt die passende Laufzeitfunktion und `runtime` in `mod.json`.

## IDs und Wiederholung

Die ID ist im jeweiligen Mod-Namensraum stabil. Dieselbe ID aktualisiert die vorhandene Mitteilung, eine neue ID erzeugt einen weiteren Eintrag. Der Feed wird lokal im konfigurierten State-Speicher abgelegt. Die Funktion terminiert, wiederholt oder verteilt Mitteilungen nicht automatisch.

## Wann eine Mitteilung passt

Melde bestätigte Ereignisse. Rufe die Funktion nicht pro Frame oder in schnellen Schleifen auf. Für technische Diagnosen nutze das [Mod-Logging](#doc-api-logging). Details zum gespeicherten Format stehen in [modloader-content.md](https://github.com/bonsaibauer/shroudforge/blob/main/docs/sf/modloader-content.md).
