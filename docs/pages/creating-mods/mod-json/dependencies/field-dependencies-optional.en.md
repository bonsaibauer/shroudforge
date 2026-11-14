---
{
  "id": "field-dependencies-optional",
  "locale": "en",
  "title": "The `dependencies[].optional` field",
  "summary": "Whether a missing or incompatible dependency blocks startup.",
  "navigation": {
    "path": [
      "mod.json",
      "Dependencies",
      "Dependency fields"
    ]
  }
}
---

# The `dependencies[].optional` field

Whether a missing or incompatible dependency blocks startup.

## Type and requirement

`boolean` · optional

## Allowed values and limits

- When true, this mod may start if the dependency is unavailable. Defaults to false.

## Example

```json
{
  "dependencies": [
    {
      "id": "helper.mod",
      "version": "^1.0.0",
      "optional": true
    }
  ]
}
```

## Source

Schema: [mod.json](https://shroudforge.net/schemas/1.8.0/manifest.schema.json). The package reader checks additional relationships between fields.
