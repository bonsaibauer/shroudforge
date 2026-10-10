# The `settings.<key>.minLength` field

Minimum length of a text value.

## Type and requirement

`integer` · optional

## Allowed values and limits

- minimum: `0`

## Example

```json
{
  "settings": {
    "example": {
      "value": "hello",
      "control": "text",
      "minLength": 3
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
