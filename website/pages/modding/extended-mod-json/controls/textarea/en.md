# The `textarea` control

For longer text edited across multiple lines. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": "Longer text", "label": "Multi-line text field", "control": "textarea"}
  }
}
```

The value in `value` should match the selected control. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
