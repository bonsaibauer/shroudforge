---
{
  "id": "field-license",
  "locale": "en",
  "title": "The `license` field",
  "summary": "License identifier when the mod declares a license.",
  "navigation": {
    "path": [
      "mod.json",
      "Basic fields"
    ]
  }
}
---

# The `license` field

License identifier when the mod declares a license.

## Type and requirement

`string | null` · optional

## Allowed values and limits

- Optional license identifier (for example, MIT). Use null when no license is declared.

## Example

```json
{
  "license": "example"
}
```

## Source

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The package reader checks additional relationships between fields.
