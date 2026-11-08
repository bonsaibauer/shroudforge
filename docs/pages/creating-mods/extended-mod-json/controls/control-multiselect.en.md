---
{
  "id": "control-multiselect",
  "locale": "en",
  "title": "The `multiselect` control",
  "summary": "Allows several values. value is a list and options defines the available items.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Controls",
      "Control types"
    ]
  }
}
---

# The `multiselect` control

Allows several values. `value` is a list and `options` defines the available items. Set it as `control` on a setting. Lists with more than 12 options gain search, an only-selected filter, a count, and a bounded scroll area. Search covers every category.

```json
{
  "settings": {
    "example": {"value": ["easy"], "label": "Multi-select", "control": "multiselect", "options": {"easy": {"label": "Easy", "group": "General"}, "hard": {"label": "Hard", "group": "Advanced"}}}
  }
}
```

Option keys are the stored values. Option labels can stay strings or use `{ "label": "Easy", "group": "General" }` to show category tabs. For `multiselect`, `value` must be a list. Mod locale files translate category tab names through `settings.<key>.optionGroups`. See [settings.<key>.control](#doc-field-settings-key-control) for the schema rules.
