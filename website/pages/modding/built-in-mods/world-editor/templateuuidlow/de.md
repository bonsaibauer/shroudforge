# Untere Hälfte der Template-UUID in Hexadezimal

Untere hexadezimale Hälfte der nativen Template-UUID für die erweiterte Aktion Queue prop spawn.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("templateUuidLow", fallback)`: `templateUuidLow`.
- Datentyp: `string`.
- Startwert: `"0000000000000000"`.
- Textwert. Das Mod-Paket legt keine weitere Längenbegrenzung fest.
- Das Steuerelement wird aus dem Werttyp abgeleitet.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("templateUuidLow", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
