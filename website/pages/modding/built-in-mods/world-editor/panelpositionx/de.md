# Fensterposition X, minus eins zentriert

Horizontale Position relativ zum Spielfenster. Der Wert −1 zentriert das Fenster, nichtnegative Werte legen den linken Abstand fest.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("panelPositionX", fallback)`: `panelPositionX`.
- Datentyp: `number`.
- Startwert: `-1`.
- Zahlenbereich: -1 bis 8192.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `-1`.
- Maximum: `8192`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("panelPositionX", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
