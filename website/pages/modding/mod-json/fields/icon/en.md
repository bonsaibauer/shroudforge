# The `icon` field

Icon file in the package root or under assets/.

## Type and requirement

`string | null` · optional

## Allowed values and limits

- pattern: `^(?:assets/[A-Za-z0-9._/-]+|[A-Za-z0-9_-][A-Za-z0-9._-]*)$`
- Optional mod icon filename in the package root or path under assets/. Use null when the mod has no icon.

## Example

```json
{
  "icon": "example"
}
```

## Source

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). The package reader checks additional relationships between fields.
