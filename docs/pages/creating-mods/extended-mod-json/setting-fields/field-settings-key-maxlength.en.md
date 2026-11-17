---
{
  "id": "field-settings-key-maxlength",
  "locale": "en",
  "title": "The `settings.<key>.maxLength` field",
  "summary": "Maximum length of a text value.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Settings",
      "Setting fields"
    ]
  }
}
---

# The `settings.<key>.maxLength` field

Maximum length of a text value.

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
      "maxLength": 5
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
