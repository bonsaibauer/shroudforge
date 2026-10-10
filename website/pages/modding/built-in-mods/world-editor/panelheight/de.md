# Höhe des Blueprint-Fensters

Höhe des World-Editor-Fensters in logischen Pixeln. Änderungen gelten während des laufenden Spiels.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("panelHeight", fallback)`: `panelHeight`.
- Datentyp: `number`.
- Startwert: `190`.
- Zahlenbereich: 140 bis 400.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `140`.
- Maximum: `400`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("panelHeight", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
