# The `multiselect` control

Allows several values. value is a list and options defines the available items. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": ["easy"], "label": "Multi-select", "control": "multiselect", "options": {"easy": "Easy", "hard": "Hard"}}
  }
}
```

Option keys are the stored values. For `multiselect`, `value` must be a list. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
