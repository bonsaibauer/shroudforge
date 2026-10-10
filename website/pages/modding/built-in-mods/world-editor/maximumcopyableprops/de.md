# Maximale Props pro Blueprint

Maximale Prop-Anzahl pro Blueprint. Aufnahme, Speichern, Laden und Einfügen lehnen größere Blueprints ab, statt Props still zu entfernen.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("maximumCopyableProps", fallback)`: `maximumCopyableProps`.
- Datentyp: `number`.
- Startwert: `60000`.
- Zahlenbereich: 1 bis 1000000.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Minimum: `1`.
- Maximum: `1000000`.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("maximumCopyableProps", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
