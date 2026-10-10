# Props am Einfügeziel

Bestimmt, ob Props am Ziel erhalten bleiben oder sicher überprüfte, sich überschneidende Props ersetzt werden.

## Bedeutung und Standard

Weltbereiche kopieren, drehen, speichern und einfügen.

- Mod: World Editor (`world-editor`).
- Schlüssel für `shroudforge.settings.get("targetPropMode", fallback)`: `targetPropMode`.
- Datentyp: `string`.
- Startwert: `"keep"`.
- Erlaubte Werte: `keep` für Vorhandene Props behalten, `replace` für Props am Ziel ersetzen, wenn sie sich überschneiden.
- Das Steuerelement wird aus dem Werttyp abgeleitet.
- Optionen: `keep` (Vorhandene Props behalten), `replace` (Props am Ziel ersetzen, wenn sie sich überschneiden).

## Woher die Angaben kommen

Diese Seite basiert auf `mods/world-editor/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("targetPropMode", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/src/mod.lua).

[Mod-Anleitung](https://github.com/bonsaibauer/shroudforge/blob/main/mods/world-editor/README.md)
