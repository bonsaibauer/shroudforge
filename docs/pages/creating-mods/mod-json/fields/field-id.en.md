---
{
  "id": "field-id",
  "locale": "en",
  "title": "The `id` field",
  "summary": "Stable identifier that other mods can depend on.",
  "navigation": {
    "path": [
      "mod.json",
      "Basic fields"
    ]
  }
}
---

# The `id` field

Stable identifier that other mods can depend on.

## Type and requirement

`string` · required

## Allowed values and limits

- maximum length: `80`
- pattern: `^[A-Za-z0-9._-]+$`

## Example

```json
{
  "id": "author.example-mod"
}
```

## Source

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The package reader checks additional relationships between fields.
