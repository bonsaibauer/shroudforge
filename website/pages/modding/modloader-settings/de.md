# Modloader-Einstellungen

Diese Einstellungen betreffen die Modloader-Oberfläche, integrierte Module und den Speicherort ihrer Daten. Sie liegen in `shroudforge/config/modloader-config.json`. Sie sind nicht die Mod-Einstellungen aus `extended.mod.json`.

Die Liste der einzelnen Felder wird aus `loader.schema.json` und den Standardwerten generiert. Jede Einstellung hat hier eine eigene Markdown-Seite. Nicht aufgeführt sind interne Aktions- und Sitzungswerte wie `requestId` oder reine Fensterpositionen.

## Bereiche

- **Allgemein**: Sprache, Benutzername und Anzeigeverhalten.
- **Protokollierung**: Mindeststufe für gespeicherte Logs.
- **Debug Console**: Quelle, Filter, Tastenkürzel und Leselimit.
- **Modloader- und World-Editor-UI**: Aktivierung und Aktualisierungsverhalten.
- **Netzwerk**: Dedicated-Server-Fallback und optionale SteamID64-Freigabeliste.
- **Runtime-Diagnose**: Bereiche, Intervall, Dauer und Langsamkeitsschwelle.
- **Updates**: Release-Kanal und Prüfintervall.
- **Speicherorte**: Mods, State, Logs, Cache, Exporte und Laufzeitdaten.

Änderungen an `minimumLevel` bestimmen, was gespeichert wird. Ein Filter in der Debug Console ändert nicht den Inhalt der Logdatei.
