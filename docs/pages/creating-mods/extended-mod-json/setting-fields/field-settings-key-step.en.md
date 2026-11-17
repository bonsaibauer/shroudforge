---
{
  "id": "field-settings-key-step",
  "locale": "en",
  "title": "The `settings.<key>.step` field",
  "summary": "Increment for number and slider inputs.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Settings",
      "Setting fields"
    ]
  }
}
---

# The `settings.<key>.step` field

Increment for number and slider inputs.

## Type and requirement

`number` · optional

## Allowed values and limits

- greater than: `0`

## Example

```json
{
  "settings": {
    "example": {
      "value": 1,
      "control": "number",
      "step": 0.1
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
