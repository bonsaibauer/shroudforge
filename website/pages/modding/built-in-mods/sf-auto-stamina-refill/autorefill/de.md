# Ausdauer automatisch auffüllen

Schaltet die wiederholte Auffüllung der Ausdauer bis zum Maximum ein oder aus.

## Bedeutung und Standard

Füllt die Ausdauer wiederholt bis zum Maximum auf.

- Mod: SF Auto Stamina Refill (`sf-auto-stamina-refill`).
- Schlüssel für `shroudforge.settings.get("autoRefill", fallback)`: `autoRefill`.
- Datentyp: `boolean`.
- Startwert: `true`.
- Erlaubte Werte: `true` oder `false`.
- Das Steuerelement wird aus dem Werttyp abgeleitet.

## Woher die Angaben kommen

Diese Seite basiert auf `mods/sf-auto-stamina-refill/extended.mod.json`. Lua liest den aktuellen Wert mit `shroudforge.settings.get("autoRefill", fallback)`. Spieleränderungen speichert der Modloader in der Erweiterungsdatei. [Lua-Quelldatei](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/sf-auto-stamina-refill/src/mod.lua).
