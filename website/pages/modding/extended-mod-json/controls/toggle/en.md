# The `toggle` control

Displays an on/off switch for a boolean value. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": true, "label": "Toggle", "control": "toggle"}
  }
}
```

The value in `value` should match the selected control. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
