# The `authors` field

Names of the people who created the mod.

## Type and requirement

`array` · optional

## Allowed values and limits

- Items must be unique.

## Example

```json
{
  "authors": [
    "Your Name"
  ]
}
```

## Source

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). The package reader checks additional relationships between fields.
