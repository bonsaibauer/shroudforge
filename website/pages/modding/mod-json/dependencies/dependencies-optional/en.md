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

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). The package reader checks additional relationships between fields.
