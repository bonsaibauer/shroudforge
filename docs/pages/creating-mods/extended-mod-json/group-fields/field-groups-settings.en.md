---
{
  "id": "field-groups-settings",
  "locale": "en",
  "title": "The `groups[].settings` field",
  "summary": "Keys of existing mod settings displayed in this group.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Groups and actions",
      "Group fields"
    ]
  }
}
---

# The `groups[].settings` field

Keys of existing mod settings displayed in this group.

## Type and requirement

`array` · optional

## Allowed values and limits

- Items must be unique.

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [
        "enabledFeature"
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
