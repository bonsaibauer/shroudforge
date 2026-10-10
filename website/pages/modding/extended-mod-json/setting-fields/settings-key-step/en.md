# The `settings.<key>.step` field

Increment for number and slider inputs.

## Type and requirement

`number` · optional

## Allowed values and limits

- greater than: `0`

## Example

```json
{
  "settings": {
    "example": {
      "value": 1,
      "control": "number",
      "step": 0.1
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
