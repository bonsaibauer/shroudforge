# The `settings.<key>.max` field

Largest accepted input value.

## Type and requirement

`number` · optional

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "settings": {
    "example": {
      "value": 1,
      "control": "number",
      "max": 1
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
