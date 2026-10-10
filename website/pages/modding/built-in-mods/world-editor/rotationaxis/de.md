# Obere Blueprint-Achse

Obere Achse neuer Aufnahmen. Sie wird im Blueprint gespeichert, bereits gespeicherte Blueprints behalten ihre eigene Achse.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("rotationAxis", fallback)`: `rotationAxis`.
- Datentyp: `string`.
- Startwert: `"y"`.
- Erlaubte Werte: `x` für X-Achse, `y` für Y-Achse, `z` für Z-Achse.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Optionen: `x` (X-Achse), `y` (Y-Achse), `z` (Z-Achse).

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("rotationAxis", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
