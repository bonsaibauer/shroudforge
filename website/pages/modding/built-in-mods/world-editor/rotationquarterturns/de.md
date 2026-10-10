# Drehung vor dem Einfügen

Anfangsdrehung des aktiven Blueprints in Vierteldrehungen. 0, 1, 2 und 3 entsprechen 0°, 90°, 180° und 270°. F3 erhöht den Wert im Spiel.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("rotationQuarterTurns", fallback)`: `rotationQuarterTurns`.
- Datentyp: `number`.
- Startwert: `0`.
- Zahlenbereich: 0 bis 3.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `0`.
- Maximum: `3`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("rotationQuarterTurns", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
