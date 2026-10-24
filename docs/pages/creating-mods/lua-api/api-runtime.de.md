---
{
  "id": "api-runtime",
  "locale": "de",
  "title": "Laufzeit, Client und Server",
  "summary": "Laufzeit-Mods starten in einem bestimmten Spielprozess. Native Spielfunktionen hängen zusätzlich vom Spielbuild und freigegebenem Profil ab.",
  "navigation": {
    "path": [
      "Lua-API-Anleitungen",
      "Funktionsanleitungen"
    ]
  },
  "apiPrefixes": [
    "runtime.",
    "RuntimeNativeBinding.",
    "RuntimeFunctionModifier.",
    "RuntimeFeatureStatus."
  ]
}
---

# Laufzeit, Client und Server

Die Lua-Laufzeit führt `src/mod.lua` in dem Prozess aus, der zu `targets` passt. Client heißt Spielprozess des Spielers, Server heißt Dedicated-Server-Prozess. Siehe [targets](#doc-targets).

## Prozessziel ist keine Multiplayer-Synchronisierung

`targets` steuert, wo der Code läuft. Es überträgt keine Werte an andere Clients und repliziert keine Weltänderung. Für Netzwerkfunktionen müssen passende APIs und eine explizite serverseitige Verarbeitung verwendet werden.

## Laufzeit-Callbacks und native Unterstützung

Die Mod muss benötigte Laufzeitfähigkeiten in `mod.json` deklarieren. Manche Spielzugriffe verwenden KFC Runtime, das eingebaute native Modul. Diese Aufrufe benötigen einen passenden Client- oder Serverbuild und ein unterstütztes Profil. Ein erfolgreicher Lua-Start bestätigt noch nicht jede native Aktion.

Beginne mit [Lua und KFC Runtime](#runtime) und suche konkrete Symbole in der [API-Referenz](#api).

Das Beispiel [`registry-overview.lua`](https://github.com/bonsaibauer/shroudforge/blob/HEAD/docs/examples/runtime/registry-overview.lua) protokolliert einmalig eine rein lesende Runtime-Registrierungsübersicht. Kopiere es nach `src/mod.lua` in einen Testmod, der die Fähigkeit `runtime` deklariert.

Die Kategorie [Modding](#doc-modding-guide) erklärt die Abfragen für [Hauptmenüzustand](#doc-modding-main-menu) und [lokalen Charakternamen](#doc-modding-player-name) mit Voraussetzungen, Rückgabewerten und vollständigen Lua-Beispielen.
