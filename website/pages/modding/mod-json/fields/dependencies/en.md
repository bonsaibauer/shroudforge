# The `dependencies` field

Other mods whose availability or version the loader checks before startup.

## Type and requirement

`array` · optional

## Allowed values and limits

- Mods that must be available before this mod starts. Each id is another mod's manifest id. Version is a SemVer version requirement. If optional is true, the dependency does not block startup when missing, disabled, or outside the version range.

## Example

```json
{
  "dependencies": [
    {
      "id": "example",
      "version": "example"
    }
  ]
}
```

## Source

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/manifest.schema.json). The package reader checks additional relationships between fields.
