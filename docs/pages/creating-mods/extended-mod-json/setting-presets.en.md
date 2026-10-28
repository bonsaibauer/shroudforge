---
{
  "id": "setting-presets",
  "locale": "en",
  "title": "presets, set several settings at once",
  "summary": "A preset setting such as Quality sets a list of other settings to prepared values. Editing one of them by hand switches the preset to Custom.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Presets"
    ]
  }
}
---

# `presets`, set several settings at once

Many games offer a Quality choice such as Low, Medium or High that sets several detailed options together. `presets` gives the same behavior to a mod's settings in the Modloader.

```json
{
  "settings": {
    "quality": {
      "value": "medium",
      "label": "Quality",
      "options": {"low": "Low", "medium": "Medium", "custom": "Custom"}
    },
    "viewRange": {"value": 128, "label": "View range", "min": 32, "max": 512},
    "detail": {"value": "medium", "label": "Detail", "options": {"low": "Low", "medium": "Medium"}}
  },
  "presets": {
    "quality": {
      "custom": "custom",
      "settings": ["viewRange", "detail"],
      "values": {
        "low": {"viewRange": 64, "detail": "low"},
        "medium": {"viewRange": 128, "detail": "medium"}
      }
    }
  }
}
```

## Behavior in the Modloader

- Choosing a preset in the controller sets every listed setting to that preset's value. Choosing the same preset again applies its values again.
- The listed settings stay editable. Changing one of them by hand selects the `custom` value in the controller and keeps the other values.
- Choosing `custom` in the controller changes nothing else.
- All changed values are saved together, like any other settings change. Lua reads each setting with `shroudforge.settings.get` as usual. The controller is an ordinary setting too.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| key of `presets` | string | Key of the controller setting. It must exist in `settings` and hold a string. |
| `custom` | string · optional | Controller value that means hand-tuned settings. Default: `custom`. If the controller has `options`, this value must be one of them. |
| `settings` | array of strings | Keys of the settings the preset sets. At least one. |
| `values` | object | One entry per preset, keyed by the controller value. Each entry gives a value to every key in `settings` and to no other key. |

## Rules checked by the package reader

- A preset's settings must exist. They cannot include the controller itself, the controller of another preset, or a `keybind` setting. A setting belongs to one preset at most.
- When the controller has `options`, every option other than `custom` needs an entry in `values`, and every entry in `values` must be an option. `values` never has an entry for `custom`.
- Each preset value must be valid for its setting: type, `min` and `max`, `minLength` and `maxLength`, and `options` are checked the same way as a saved value.

The settings page is the same in the desktop window and in the window opened from the game's **Mods** menu entry. The **Hotkeys** tab edits keybinds directly, which is why keybind settings cannot belong to a preset.

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/manifest/extended.mod.schema.json). The package reader checks the relationships described above.
