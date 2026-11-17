---
{
  "id": "field-settings-key-control",
  "locale": "en",
  "title": "The `settings.<key>.control` field",
  "summary": "Input control displayed by the Modloader.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Settings",
      "Setting fields"
    ]
  }
}
---

# The `settings.<key>.control` field

Input control displayed by the Modloader.

## Type and requirement

`string` · optional

## Allowed values and limits

- values: `"toggle"`, `"checkbox"`, `"text"`, `"textarea"`, `"number"`, `"slider"`, `"select"`, `"radio"`, `"segmented"`, `"multiselect"`, `"keybind"`, `"color"`

With `"keybind"`, `value` must be one SDL binding string such as `"SDL:64"`, `"Mouse:4"` or `"Ctrl+SDL:29|Mouse:4"`. It allows at most one keyboard chord and one mouse chord, separated by `|` with the keyboard first. The empty string `""` disables the shortcut. Optional modifier prefixes appear once per chord in `Ctrl`, `Alt`, `Shift`, `Meta` order. The editor shows separate keyboard and mouse columns while saving this single value. See the [keybind control](#doc-control-keybind) for the Lua API and editing behavior.

## Example

```json
{
  "settings": {
    "example": {
      "value": 1,
      "control": "slider"
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
