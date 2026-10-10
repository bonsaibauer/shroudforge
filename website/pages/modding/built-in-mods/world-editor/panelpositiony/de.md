# Fensterposition Y ab oberem Rand

Vertikaler Abstand des World-Editor-Fensters vom oberen Rand des Spielfensters.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("panelPositionY", fallback)`: `panelPositionY`.
- Datentyp: `number`.
- Startwert: `92`.
- Zahlenbereich: 0 bis 8192.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `0`.
- Maximum: `8192`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("panelPositionY", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
