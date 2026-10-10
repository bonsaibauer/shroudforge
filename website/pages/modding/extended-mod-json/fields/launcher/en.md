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

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
