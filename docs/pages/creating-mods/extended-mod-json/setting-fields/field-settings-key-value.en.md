---
{
  "id": "field-settings-key-value",
  "locale": "en",
  "title": "The `settings.<key>.value` field",
  "summary": "Starting value and expected Lua data type of the setting.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Settings",
      "Setting fields"
    ]
  }
}
---

# The `settings.<key>.value` field

Starting value and expected Lua data type of the setting.

## Type and requirement

`boolean | string | number | array` · required

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "settings": {
    "example": {
      "value": true
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
