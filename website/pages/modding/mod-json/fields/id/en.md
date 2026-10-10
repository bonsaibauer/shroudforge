# The `id` field

Stable identifier that other mods can depend on.

## Type and requirement

`string` · required

## Allowed values and limits

- maximum length: `80`
- pattern: `^[A-Za-z0-9._-]+$`

## Example

```json
{
  "id": "author.example-mod"
}
```

## Source

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). The package reader checks additional relationships between fields.
