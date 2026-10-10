# Lua-API, Anleitungen nach Funktion

Die [durchsuchbare API-Referenz](#api) ist das vollständige Verzeichnis der implementierten Funktionen und Typen. Diese Anleitungen zeigen typische Aufgaben mit Kontext und Beispielcode.

## Häufige Aufgaben

- [Modloader-Mitteilungen veröffentlichen](#doc-api-notifications) erklärt `shroudforge.notifications.publish` und wie Nachrichten im Modloader erscheinen.
- [Mod-Einstellungen lesen](#doc-api-settings) verbindet `extended.mod.json` mit `shroudforge.settings.get`.
- [Modloader-Aktionen verbinden](#doc-api-ui-actions) verbindet die Aktions-ID mit `shroudforge.ui.on_action`.
- [Meldungen schreiben](#doc-api-logging) erklärt Log-Level und sinnvolle Meldungen.
- [Dateien und Exporte](#doc-api-files) beschreibt Paketdateien und Exportberechtigungen.
- [Laufzeit und Spielfunktionen](#doc-api-runtime) beschreibt Lifecycle, Client/Server und native Unterstützung.

## So sind API-Artikel aufgebaut

Jede Anleitung nennt Zweck, passende Situation, erforderliche Fähigkeiten, erwartete Argumente und Ergebnisse. Für alle übrigen Funktionen nutze die Suchfunktion der API-Referenz, sie verlinkt auf den Lua-Quellvertrag. API-Funktionen können EML oder ShroudForge-spezifisch sein.
