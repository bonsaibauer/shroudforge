# Neuer Name beim Umbenennen oder Duplizieren

Zielname für die Mod-Aktionen zum Umbenennen oder Duplizieren eines Blueprints.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("blueprintNewName", fallback)`: `blueprintNewName`.
- Datentyp: `string`.
- Startwert: `"my_blueprint_copy"`.
- Textlänge: 1 bis 64 Zeichen.
- Das Steuerelement wird aus dem Werttyp abgeleitet.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("blueprintNewName", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
