---
{
  "id": "field-authors",
  "locale": "en",
  "title": "The `authors` field",
  "summary": "Names of the people who created the mod.",
  "navigation": {
    "path": [
      "mod.json",
      "Basic fields"
    ]
  }
}
---

# The `authors` field

Names of the people who created the mod.

## Type and requirement

`array` · optional

## Allowed values and limits

- Items must be unique.

## Example

```json
{
  "authors": [
    "Your Name"
  ]
}
```

## Source

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The package reader checks additional relationships between fields.
