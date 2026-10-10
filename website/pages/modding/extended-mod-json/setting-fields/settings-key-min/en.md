# The `settings.<key>.min` field

Smallest accepted input value.

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
      "min": 1
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
