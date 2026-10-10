# Interner Entity-Handle

Laufzeit-Handle eines Props aus der Aktion List props. Es gilt nur für den aktuellen Prozess und ist für die erweiterten nativen Aktionen gedacht.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("entityHandle", fallback)`: `entityHandle`.
- Datentyp: `number`.
- Startwert: `0`.
- Zahlenbereich: 0 bis kein Maximum.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `0`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("entityHandle", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
