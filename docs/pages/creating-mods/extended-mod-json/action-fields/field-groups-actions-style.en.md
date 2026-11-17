---
{
  "id": "field-groups-actions-style",
  "locale": "en",
  "title": "The `groups[].actions[].style` field",
  "summary": "Appearance of the action button, without changing its behavior.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Groups and actions",
      "Action fields"
    ]
  }
}
---

# The `groups[].actions[].style` field

Appearance of the action button, without changing its behavior.

## Type and requirement

`string` · optional

## Allowed values and limits

- values: `"primary"`, `"secondary"`, `"danger"`

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
          "style": "secondary"
        }
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
