---
{
  "id": "field-inputreservations",
  "locale": "en",
  "title": "The `inputReservations` field",
  "summary": "Keys and mouse wheel the game does not receive while the mod needs them.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Fields"
    ]
  }
}
---

# The `inputReservations` field

Keys and mouse wheel the game does not receive while the mod needs them. A building mod can then use `1` for its start point while the hammer is in hand, without the game also switching the hotbar slot, and read the mouse wheel without the game zooming or scrolling.

## Type and requirement

`object` · optional

## Parts

- `contexts`: the situations the mod reports while it runs, as ID and label, for example `"hammer": "Hammer in hand"`. The label tells the player when a key is reserved.
- `keys`: per `keybind` setting, the context `when` and the defaults per device, `keyboard` and `mouse` (`false` when omitted).
- `wheels`: per boolean setting, the context `when` in which the mod reads the mouse wheel. The setting's value is the player's choice: `true` keeps the wheel from the game meanwhile, `false` shares it.
- `choices`: the player's padlocks, written by the Modloader. Do not ship it.

## Example

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
            -- The game did not receive this press while the hammer was in hand.
        end
    end,
}
```

## Behaviour

The player decides. In the mod settings, a padlock stands in front of each reservable key, per device: closed, the game does not receive the key while the context is active; open, the game receives it too. The mod's `keys` only give the first state. A key without an entry in `keys` is always shared.

A context lasts one second after the mod last reported it. Report it from every `on_update` while it holds and report `false` as soon as it ends. A mod that stops, fails or stalls thus gives the game its input back. The release of a press the game received is never kept. Escape, F9 and F10 always reach the game.

The mod still reads its reserved keys with `is_keybind_down` and `was_keybind_pressed`: ShroudForge reads them in its own keyboard process, before the game. It reads the wheel with `shroudforge.input.wheel_steps`.

A ShroudForge without input reservations ignores this top-level field, so the same mod still loads there: check for `shroudforge.input.set_context` before using it.

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/manifest/extended.mod.schema.json). The package reader checks that each context named by `when` is declared, that `keys` names `keybind` settings and `wheels` boolean settings.
