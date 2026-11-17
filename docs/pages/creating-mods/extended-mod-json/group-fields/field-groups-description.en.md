---
{
  "id": "field-groups-description",
  "locale": "en",
  "title": "The `groups[].description` field",
  "summary": "Short text shown on the mod page.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Groups and actions",
      "Group fields"
    ]
  }
}
---

# The `groups[].description` field

Short text shown on the mod page.

## Type and requirement

`string` · optional

## Allowed values and limits

- maximum length: `500`

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [],
      "description": "example"
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
