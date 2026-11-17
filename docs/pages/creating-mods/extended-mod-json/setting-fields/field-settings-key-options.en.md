---
{
  "id": "field-settings-key-options",
  "locale": "en",
  "title": "The `settings.<key>.options` field",
  "summary": "Allowed choices and the labels players see.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Settings",
      "Setting fields"
    ]
  }
}
---

# The `settings.<key>.options` field

Allowed choices and the labels players see.

## Type and requirement

`object` · optional

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "settings": {
    "example": {
      "value": "easy",
      "control": "select",
      "options": {
        "easy": "Easy",
        "hard": "Hard"
      }
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
