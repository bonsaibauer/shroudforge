# EML-Mods verstehen und migrieren

`mod.json` basiert auf dem EML-Manifestformat. ShroudForge nutzt weiterhin dessen Mod-ID, Name, Version, Abhängigkeiten und Berechtigungsangaben. Zusätzliche ShroudForge-Werte stehen in `extended.mod.json`.

## Herkunft und Launcher-Badge

Ein EML-Mod ohne Erweiterungsdatei wird als älteres EML-Paket behandelt. Wenn ShroudForge beim Speichern eine Erweiterung anlegt, setzt es `launcher: "EML"`, damit die Herkunft erhalten bleibt. Für ShroudForge-Mods kann `launcher` fehlen.

## Client und Server

EML-Pakete ohne explizites `targets` laufen standardmäßig auf Client und Server, auch wenn die Erweiterungsdatei `launcher: "EML"` enthält. Für neue ShroudForge-Pakete ist der Standard Client. Schreibe `targets` ausdrücklich, wenn ein anderer Prozess gebraucht wird. Zielprozesse garantieren keine Netzwerkübertragung. Siehe [targets](#doc-targets).

## Berechtigungen

Prüfe, welche Fähigkeiten der Mod tatsächlich verwendet. `runtime-register-dll` bezieht sich auf eine DLL aus dem Mod-Paket und ist nicht die eingebaute ShroudForge `kfc-runtime.dll`. Die Berechtigung plant die EML-DLL-Registrierung für die Laufzeit ein.

## Sichere Migration

1. Sichere den Mod-Ordner.
2. Lass `mod.json` unverändert, sofern keine belegte Anpassung nötig ist.
3. Ergänze die optionale `extended.mod.json` für ShroudForge-Einstellungen.
4. Setze `launcher: "EML"` und das gewünschte `targets` ausdrücklich.
5. Prüfe den Mod im Modloader und im passenden Client- oder Serverlog.

Eine erfolgreiche Paketprüfung bedeutet nicht automatisch, dass jede EML-Funktion oder jede native Operation mit jedem Spielbuild kompatibel ist. Die [durchsuchbare API-Referenz](#api) unterscheidet EML- und ShroudForge-Funktionen.
