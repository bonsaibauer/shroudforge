---
{
  "id": "field-settings-key-minlength",
  "locale": "en",
  "title": "The `settings.<key>.minLength` field",
  "summary": "Minimum length of a text value.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Settings",
      "Setting fields"
    ]
  }
}
---

# The `settings.<key>.minLength` field

Minimum length of a text value.

## Type and requirement

`integer` · optional

## Allowed values and limits

- minimum: `0`

## Example

```json
{
  "settings": {
    "example": {
      "value": "hello",
      "control": "text",
      "minLength": 3
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
