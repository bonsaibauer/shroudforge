# SF Production Time

Legt eine Basisdauer für zeitgesteuerte Produktionsrezepte fest. Weltgeschwindigkeitseinstellungen gelten weiterhin.

## Paketangaben

- Mod-ID: `sf-production-time`
- Zielprozesse: Client und Dedicated Server
- Laufzeitberechtigung: nein
- Deklarierte spielerseitige Einstellungen: 1

## Einstellungen

## Produktionszeit

Lege vor dem Spielstart denselben Wert auf Client und Host oder Server fest. Weltgeschwindigkeitseinstellungen multiplizieren diese Basisdauer. Sofortrezepte und Inventarmengen bleiben unverändert.

- [Basis-Produktionszeit in Sekunden](#doc-builtin-sf-production-time-seconds)

Die Einstellungen werden in `extended.mod.json` des Mod-Pakets gespeichert. Der Mod liest sie über `shroudforge.settings.get(key, fallback)`. Weitere Details stehen in der [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/sf-production-time/src/mod.lua).
