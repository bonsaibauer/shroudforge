# Auswahlgröße Y als manueller Ersatzwert

Manuelle Ausdehnung der Aufnahme entlang Y in Voxel-Zellen. Wird nur verwendet, wenn keine vollständige Auswahl über Cursor-Ecken vorliegt.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("sizeY", fallback)`: `sizeY`.
- Datentyp: `number`.
- Startwert: `8`.
- Zahlenbereich: 1 bis 65536.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `1`.
- Maximum: `65536`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("sizeY", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
