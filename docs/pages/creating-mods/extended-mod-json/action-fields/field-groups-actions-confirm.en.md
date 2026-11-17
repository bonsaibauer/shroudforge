---
{
  "id": "field-groups-actions-confirm",
  "locale": "en",
  "title": "The `groups[].actions[].confirm` field",
  "summary": "Optional confirmation prompt before running an action.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Groups and actions",
      "Action fields"
    ]
  }
}
---

# The `groups[].actions[].confirm` field

Optional confirmation prompt before running an action.

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
      "actions": [
        {
          "id": "reset",
          "label": "Reset",
          "confirm": "example"
        }
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
