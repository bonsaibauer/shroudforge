---
{
  "id": "api-guides",
  "locale": "de",
  "title": "Lua-API, Anleitungen nach Funktion",
  "summary": "Die API-Referenz listet alle verfügbaren Symbole. Diese Anleitungen erklären häufige Aufgaben mit Ablauf, Voraussetzungen und vollständigen Beispielen.",
  "navigation": {
    "path": [
      "Lua-API-Anleitungen"
    ],
    "overview": true,
    "label": "Übersicht"
  }
}
---

# Lua-API, Anleitungen nach Funktion

Die [durchsuchbare API-Referenz](#api) ist das vollständige Verzeichnis der implementierten Funktionen und Typen. Diese Anleitungen zeigen typische Aufgaben mit Kontext und Beispielcode.

## Häufige Aufgaben

- [Modloader-Mitteilungen veröffentlichen](#doc-api-notifications) erklärt `shroudforge.notifications.publish` und wie Nachrichten im Modloader erscheinen.
- [Mod-Einstellungen lesen](#doc-api-settings) verbindet `extended.mod.json` mit `shroudforge.settings.get`.
- [Modloader-Aktionen verbinden](#doc-api-ui-actions) verbindet die Aktions-ID mit `shroudforge.ui.on_action`.
- [Meldungen schreiben](#doc-api-logging) erklärt Log-Level und sinnvolle Meldungen.
- [Dateien und Exporte](#doc-api-files) beschreibt Paketdateien und Exportberechtigungen.
- [Laufzeit und Spielfunktionen](#doc-api-runtime) beschreibt Lifecycle, Client/Server und native Unterstützung.
- [Hauptmenü erkennen](#doc-modding-main-menu) erklärt `runtime.is_main_menu()` und unbekannte Ladeübergänge.
- [Charakternamen lesen](#doc-modding-player-name) erklärt `runtime.player.name()`, Standardnamen und den gemeinsamen Cache.

## So sind API-Artikel aufgebaut

Jede Anleitung nennt Zweck, passende Situation, erforderliche Fähigkeiten, erwartete Argumente und Ergebnisse. Für alle übrigen Funktionen nutze die Suchfunktion der API-Referenz, sie verlinkt auf den Lua-Quellvertrag. API-Funktionen können EML oder ShroudForge-spezifisch sein.
