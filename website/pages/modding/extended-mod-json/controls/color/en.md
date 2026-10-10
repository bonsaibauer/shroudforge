# The `color` control

For a color value edited through a color input in Modloader. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": "#d0ae6d", "label": "Color picker", "control": "color"}
  }
}
```

The value in `value` should match the selected control. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
