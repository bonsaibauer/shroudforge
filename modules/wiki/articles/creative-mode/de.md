# Multi DLL Manager

> 🧪 **BETA**

Creative Mode von Phoenix1355 wird automatisch erkannt, sobald sein Modordner unter `mods` liegt. Du musst keine `mod.json` bearbeiten und die DLL nicht manuell in der DLL-Verwaltung einrichten.

## Creative Mode installieren

1. Lade Creative Mode von [Nexus Mods](https://www.nexusmods.com/enshrouded/mods/146) herunter und entpacke das Archiv.
2. Lege den Creative-Modordner in den ShroudForge-Ordner `mods`. ShroudForge erkennt den Mod automatisch.

## Creative Mode im Spiel öffnen

- Drücke **Insert**, um das Creative-Menü zu öffnen oder zu schließen.
- Drücke **G**, um den Charakterflug ein- oder auszuschalten.
- Im Menü kannst du Gegenstände auswählen und sie deinem Inventar oder einer Aktionsleiste hinzufügen.
- Die Mod-Einstellungen und Tastenbelegungen findest du im Settings-Tab des Creative-Menüs.

## Andere Mods mit DirectInput-DLLs

Bei anderen Mods mit einer DirectInput-DLL, meist `dinput8.dll`, ist der jeweilige Modordner unter `mods/` der empfohlene Ablageort. Behalte den Dateinamen bei. Details zu den DLL-Varianten findest du im vorhandenen [DLL-Manager-Wikiartikel](#wiki/dll-manager).

## Einstellungen unter Kompatibilität

Für andere Mods, die eine DLL zum Start benötigen, öffne im Modloader **Kompatibilität → DLL-Verwaltung**. Dort kannst du:

- den DLL Manager global ein- oder ausschalten und das Laden pro Mod aktivieren oder deaktivieren
- ein automatisches oder ein bestimmtes Kompatibilitätsprofil auswählen
- bei einer manuellen Auswahl die Einstiegs-DLLs festlegen und ihre Startmethode wählen
- die Reihenfolge der ausgewählten DLLs ändern
- als Ladeziel Spielclient, Spielserver oder beide auswählen
- die manuelle DLL-Auswahl ausdrücklich bestätigen
- DLL-Dateinamen von Erkennung und Laden ausschließen
- vorhandene `dinput8-chain.ini`-Einträge und den Ladestatus prüfen

Kompatibilitätseinstellungen ändern keine installierten Moddateien. Änderungen am DLL-Laden gelten beim nächsten Spielstart. Eine Anzeige „DLL geladen“ bestätigt den Ladevorgang, aber nicht automatisch jede Modfunktion.

## Weitere Wiki-Artikel

- [DLL Manager](#wiki/dll-manager)
- [Mod-Erkennung](#wiki/mod-detection)
- [Creative Mode herunterladen](https://www.nexusmods.com/enshrouded/mods/146)
