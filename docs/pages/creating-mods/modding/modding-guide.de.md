---
{
  "id": "modding-guide",
  "locale": "de",
  "title": "Modding mit der Lua-Runtime",
  "summary": "Praktische Lua-Anleitungen zum Hauptmenüzustand, Charakternamen und zur Lokalisierung von Mods.",
  "navigation": {
    "path": ["Modding"],
    "overview": true,
    "label": "Übersicht"
  }
}
---

# Modding mit der Lua-Runtime

Diese Anleitungen zeigen, wie eine aktive Mod den Hauptmenüzustand und den ausgewählten lokalen Charakternamen abfragt. Beide Informationen stammen aus derselben nativen Runtime, die auch die Modloader-UI verwendet.

| Aufgabe | Lua-Funktion | Anleitung |
| --- | --- | --- |
| Hauptmenü erkennen | `runtime.is_main_menu()` | [Hauptmenüzustand abfragen](#doc-modding-main-menu) |
| Ausgewählten Charakternamen lesen | `runtime.player.name()` | [Lokalen Charakternamen abfragen](#doc-modding-player-name) |
| Mod-Texte und Einstellungen übersetzen | `shroudforge.locale()` und `shroudforge.i18n.get()` | [Mod-Texte lokalisieren](#doc-modding-locales) |

Die Mod benötigt die [Berechtigung `runtime`](#doc-capability-runtime) und einen aktiven Laufzeit-Callback. Ein Laufzeit-Callback kann bereits im Hauptmenü arbeiten. Das Feld `runtime.phase == "ingame"` bezeichnet die gestartete Mod-Laufzeit und bestätigt keinen Weltbeitritt.

Die Artikel enthalten Voraussetzungen, Rückgabewerte und vollständige Lua-Beispiele. Weitere Funktionen stehen in der [API-Referenz](#api) und den [Lua-API-Anleitungen](#doc-api-guides).
