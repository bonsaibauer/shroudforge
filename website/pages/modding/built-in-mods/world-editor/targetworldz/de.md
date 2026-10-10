# Weltposition Z für Props-only-Blueprints

Weltkoordinate Z als manuelles Einfügeziel für Props-only-Blueprints, wenn weder Cursor noch markiertes Ziel verwendet werden.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("targetWorldZ", fallback)`: `targetWorldZ`.
- Datentyp: `number`.
- Startwert: `0`.
- Zahlenwert. Das Mod-Paket legt keine weitere Grenze fest.
- Das Steuerelement wird aus dem Werttyp abgeleitet.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("targetWorldZ", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
