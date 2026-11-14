---
{
  "id": "field-dependencies-version",
  "locale": "en",
  "title": "The `dependencies[].version` field",
  "summary": "SemVer range accepted for the required mod's version.",
  "navigation": {
    "path": [
      "mod.json",
      "Dependencies",
      "Dependency fields"
    ]
  }
}
---

# The `dependencies[].version` field

SemVer range accepted for the required mod's version.

## Type and requirement

`string` · required

## Allowed values and limits

- SemVer version requirement, for example ^1.2.0 or >=1.2.0, <2.0.0.

## Example

```json
{
  "dependencies": [
    {
      "id": "helper.mod",
      "version": "^1.2.0"
    }
  ]
}
```

## Source

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The package reader checks additional relationships between fields.
