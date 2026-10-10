# Voxel-Modus beim Einfügen

Bestimmt, wie Voxel beim Einfügen zusammengeführt werden. `replace` ersetzt die Zielzellen, `add` fügt nur belegte Quellzellen hinzu.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("pasteVoxelMode", fallback)`: `pasteVoxelMode`.
- Datentyp: `string`.
- Startwert: `"replace"`.
- Erlaubte Werte: `replace` für Voxel am Ziel ersetzen, `add` für Belegte Voxel aus der Quelle hinzufügen.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Optionen: `replace` (Voxel am Ziel ersetzen), `add` (Belegte Voxel aus der Quelle hinzufügen).

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("pasteVoxelMode", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/world-editor/README.md)
