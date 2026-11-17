---
{
  "id": "field-launcher",
  "locale": "en",
  "title": "The `launcher` field",
  "summary": "Package origin used in the Modloader display.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Fields"
    ]
  }
}
---

# The `launcher` field

Package origin used in the Modloader display.

## Type and requirement

`string` · optional

## Allowed values and limits

- values: `"EML"`, `"SF"`
- Launcher that originally identified this mod. Omit for ShroudForge mods. Use EML to preserve EML provenance.

## Example

```json
{
  "launcher": "EML"
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
