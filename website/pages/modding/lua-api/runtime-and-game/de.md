# Laufzeit, Client und Server

Die Lua-Laufzeit führt `src/mod.lua` in dem Prozess aus, der zu `targets` passt. Client heißt Spielprozess des Spielers, Server heißt Dedicated-Server-Prozess. Siehe [targets](#doc-targets).

## Prozessziel ist keine Multiplayer-Synchronisierung

`targets` steuert, wo der Code läuft. Es überträgt keine Werte an andere Clients und repliziert keine Weltänderung. Für Netzwerkfunktionen müssen passende APIs und eine explizite serverseitige Verarbeitung verwendet werden.

## Laufzeit-Callbacks und native Unterstützung

Die Mod muss benötigte Laufzeitfähigkeiten in `mod.json` deklarieren. Manche Spielzugriffe verwenden KFC Runtime, das eingebaute native Modul. Diese Aufrufe benötigen einen passenden Client- oder Serverbuild und ein unterstütztes Profil. Ein erfolgreicher Lua-Start bestätigt noch nicht jede native Aktion.

Beginne mit [Lua und KFC Runtime](#runtime) und suche konkrete Symbole in der [API-Referenz](#api).
