---
{
  "id": "field-groups-actions-id",
  "locale": "en",
  "title": "The `groups[].actions[].id` field",
  "summary": "Unique action name that the mod registers in Lua.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Groups and actions",
      "Action fields"
    ]
  }
}
---

# The `groups[].actions[].id` field

Unique action name that the mod registers in Lua.

## Type and requirement

`string` · required

## Allowed values and limits

- pattern: `^[A-Za-z0-9._-]{1,80}$`

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "actions": [
        {
          "id": "resetFeature",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
