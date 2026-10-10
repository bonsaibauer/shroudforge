# Quellzelle Z als manueller Ersatzwert

Manueller Z-Koordinatenwert der Quellzelle für eine Aufnahme ohne vollständig markierte Cursor-Ecken. Normalerweise die Auswahl mit F5 markieren.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("sourceZ", fallback)`: `sourceZ`.
- Datentyp: `number`.
- Startwert: `0`.
- Zahlenwert. Das Mod-Paket legt keine weitere Grenze fest.
- Das Steuerelement wird aus dem Werttyp abgeleitet.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("sourceZ", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
