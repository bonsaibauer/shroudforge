---
{
  "id": "field-version",
  "locale": "en",
  "title": "The `version` field",
  "summary": "Version of this mod release using the basic SemVer format.",
  "navigation": {
    "path": [
      "mod.json",
      "Basic fields"
    ]
  }
}
---

# The `version` field

Version of this mod release using the basic SemVer format.

## Type and requirement

`string` · required

## Allowed values and limits

- pattern: `^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$`

## Example

```json
{
  "version": "1.0.0"
}
```

## Source

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The package reader checks additional relationships between fields.
