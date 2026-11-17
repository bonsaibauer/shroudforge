---
{
  "id": "field-targets",
  "locale": "en",
  "title": "The `targets` field",
  "summary": "Selects the client process, the Dedicated Server, or both as mod execution targets.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Targets"
    ]
  }
}
---

# The `targets` field

Selects the client process, the Dedicated Server, or both as mod execution targets.

## Type and requirement

`array` · optional

## Allowed values and limits

- list items: `"client"`, `"server"`
- minimum items: `1`
- Items must be unique.
- Processes this mod runs in. Legacy EML packages without targets default to client and server. ShroudForge packages default to client.

## Example

```json
{
  "targets": [
    "client"
  ]
}
```

## Source

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). The package reader checks additional relationships between fields.
