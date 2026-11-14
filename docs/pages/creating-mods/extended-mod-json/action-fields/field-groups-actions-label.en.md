---
{
  "id": "field-groups-actions-label",
  "locale": "en",
  "title": "The `groups[].actions[].label` field",
  "summary": "Player-facing name displayed in the Modloader.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Groups and actions",
      "Action fields"
    ]
  }
}
---

# The `groups[].actions[].label` field

Player-facing name displayed in the Modloader.

## Type and requirement

`string` · required

## Allowed values and limits

- minimum length: `1`
- maximum length: `120`

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "actions": [
        {
          "id": "reset",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
