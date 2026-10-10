# Breite des Blueprint-Fensters

Breite des World-Editor-Fensters in logischen Pixeln. Änderungen gelten während des laufenden Spiels.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("panelWidth", fallback)`: `panelWidth`.
- Datentyp: `number`.
- Startwert: `1180`.
- Zahlenbereich: 760 bis 1920.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `760`.
- Maximum: `1920`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("panelWidth", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
