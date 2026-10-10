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

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). The package reader checks additional relationships between fields.
