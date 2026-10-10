# The `select` control

For exactly one choice from options. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": "easy", "label": "Select list", "control": "select", "options": {"easy": "Easy", "hard": "Hard"}}
  }
}
```

Option keys are the stored values. For `multiselect`, `value` must be a list. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
