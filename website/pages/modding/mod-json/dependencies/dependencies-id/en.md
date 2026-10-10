# The `dependencies[].id` field

Exact mod ID from the required mod's mod.json.

## Type and requirement

`string` · required

## Allowed values and limits

- maximum length: `80`
- pattern: `^[A-Za-z0-9._-]+$`
- Exact id from the dependency mod's mod.json.

## Example

```json
{
  "dependencies": [
    {
      "id": "author.shared-library",
      "version": "^1.0.0"
    }
  ]
}
```

## Source

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/manifest.schema.json). The package reader checks additional relationships between fields.
