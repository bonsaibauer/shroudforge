---
{
  "id": "field-inputreservations",
  "locale": "de",
  "title": "Das Feld `inputReservations`",
  "summary": "Tasten und Mausrad, die das Spiel nicht erhält, solange die Mod sie braucht.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Felder"
    ]
  }
}
---

# Das Feld `inputReservations`

Tasten und Mausrad, die das Spiel nicht erhält, solange die Mod sie braucht. Eine Bau-Mod kann so `1` für ihren Startpunkt nutzen, während der Hammer in der Hand ist, ohne dass das Spiel zugleich den Schnellzugriffsplatz wechselt. Sie kann auch das Mausrad lesen, ohne dass das Spiel zoomt oder blättert.

## Typ und Pflicht

`object` · optional

## Teile

- `contexts`: die Situationen, die die Mod während des Laufs meldet, als Kennung und Bezeichnung, zum Beispiel `"hammer": "Hammer in hand"`. Die Bezeichnung sagt dem Spieler, wann eine Taste reserviert ist.
- `keys`: je `keybind`-Einstellung der Kontext `when` und die Vorgaben je Gerät, `keyboard` und `mouse` (ohne Angabe `false`).
- `wheels`: je boolesche Einstellung der Kontext `when`, in dem die Mod das Mausrad liest. Der Wert der Einstellung ist die Wahl des Spielers. `true` hält das Mausrad währenddessen vom Spiel fern, `false` teilt es.
- `choices`: die Schlösser des Spielers, vom Modloader geschrieben. Nicht mit der Mod ausliefern.

## Beispiel

```json
{
  "settings": {
    "keyStart": {"value": "SDL:30", "label": "Start point", "control": "keybind", "applyAt": "live"},
    "wheelDistance": {"value": true, "label": "Mouse wheel: distance", "control": "toggle", "applyAt": "live"}
  },
  "inputReservations": {
    "contexts": {"hammer": "Hammer in hand", "free": "Free point on"},
    "keys": {"keyStart": {"when": "hammer", "keyboard": true}},
    "wheels": {"wheelDistance": {"when": "free"}}
  }
}
```

```lua
return {
    on_update = function()
        if shroudforge.input.set_context then
            shroudforge.input.set_context("hammer", hammer_in_hand())
            shroudforge.input.set_context("free", free_point_on())
            distance = distance + shroudforge.input.wheel_steps("wheelDistance")
        end
        if shroudforge.input.was_keybind_pressed("keyStart") then
            -- Das Spiel hat diesen Druck nicht erhalten, solange der Hammer in der Hand war.
        end
    end,
}
```

## Verhalten

Der Spieler entscheidet. In den Mod-Einstellungen steht vor jeder reservierbaren Taste ein Schloss, je Gerät. Geschlossen erhält das Spiel die Taste nicht, solange der Kontext aktiv ist. Offen erhält das Spiel sie ebenfalls. Die `keys` der Mod geben nur den ersten Zustand vor. Eine Taste ohne Eintrag in `keys` wird immer geteilt.

Ein Kontext gilt eine Sekunde nach der letzten Meldung der Mod. Melde ihn in jedem `on_update`, solange er besteht, und melde `false`, sobald er endet. Eine Mod, die anhält, scheitert oder hängt, gibt dem Spiel so seine Eingaben zurück. Das Loslassen eines Drucks, den das Spiel erhalten hat, wird nie zurückgehalten. Escape, F9 und F10 erreichen das Spiel immer.

Die Mod liest ihre reservierten Tasten weiter mit `is_keybind_down` und `was_keybind_pressed`, denn ShroudForge liest sie in seinem eigenen Tastaturprozess vor dem Spiel. Das Mausrad liest sie mit `shroudforge.input.wheel_steps`.

Ein ShroudForge ohne Eingabereservierungen ignoriert dieses Feld der obersten Ebene. Dieselbe Mod lädt dort also weiterhin. Prüfe vor der Nutzung, ob `shroudforge.input.set_context` vorhanden ist.

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/manifest/extended.mod.schema.json). Der Paketleser prüft, dass jeder mit `when` genannte Kontext deklariert ist, dass `keys` `keybind`-Einstellungen und `wheels` boolesche Einstellungen nennt.
