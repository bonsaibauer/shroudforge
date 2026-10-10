# The `checkbox` control

Displays a boolean value as a checkbox. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": false, "label": "Checkbox", "control": "checkbox"}
  }
}
```

The value in `value` should match the selected control. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
