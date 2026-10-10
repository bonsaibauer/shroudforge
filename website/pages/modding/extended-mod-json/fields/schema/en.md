# The `$schema` field

This field configures `$schema`. The exact technical contract is defined by the schema.

## Type and requirement

`string` · optional

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "$schema": "example"
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
