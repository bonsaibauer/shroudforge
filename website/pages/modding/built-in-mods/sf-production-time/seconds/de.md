# Basis-Produktionszeit in Sekunden

Legt die Basisdauer in Sekunden für alle zeitgesteuerten Produktionsrezepte fest. Stelle denselben Wert auf Client und Host oder Server ein, bevor das Spiel startet.

## Bedeutung und Standard

Legt eine Basisdauer für zeitgesteuerte Produktionsrezepte fest. Weltgeschwindigkeitseinstellungen gelten weiterhin.

- Mod: SF Production Time (`sf-production-time`).
- Schlüssel für `shroudforge.settings.get("seconds", fallback)`: `seconds`.
- Datentyp: `number`.
- Startwert: `1`.
- Zahlenbereich: 0.1 bis 3600.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `0.1`.
- Maximum: `3600`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/sf-production-time/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("seconds", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/sf-production-time/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/sf-production-time/README.md)
