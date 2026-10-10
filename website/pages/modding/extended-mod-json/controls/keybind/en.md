# The `keybind` control

For a keyboard shortcut stored by Modloader as a key value. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": "F7", "label": "Key binding", "control": "keybind"}
  }
}
```

The value in `value` should match the selected control. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
