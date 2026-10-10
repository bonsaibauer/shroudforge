# The `radio` control

Shows every option with exactly one active choice. Set it as `control` on a setting.

```json
{
  "settings": {
    "example": {"value": "easy", "label": "Radio buttons", "control": "radio", "options": {"easy": "Easy", "hard": "Hard"}}
  }
}
```

Option keys are the stored values. For `multiselect`, `value` must be a list. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
