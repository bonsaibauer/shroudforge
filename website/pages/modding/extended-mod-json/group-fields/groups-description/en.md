# The `groups[].description` field

Short text shown on the mod page.

## Type and requirement

`string` · optional

## Allowed values and limits

- maximum length: `500`

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [],
      "description": "example"
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
