---
{
  "id": "field-schema",
  "locale": "en",
  "title": "The `$schema` field",
  "summary": "This field configures `$schema`. The exact technical contract is defined by the schema.",
  "navigation": {
    "path": [
      "mod.json",
      "Basic fields"
    ]
  }
}
---

# The `$schema` field

This field configures `$schema`. The exact technical contract is defined by the schema.

## Type and requirement

`string` · optional

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "$schema": "example"
}
```

## Source

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The package reader checks additional relationships between fields.
