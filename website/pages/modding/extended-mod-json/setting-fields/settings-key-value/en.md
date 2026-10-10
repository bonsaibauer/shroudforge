# The `settings.<key>.value` field

Starting value and expected Lua data type of the setting.

## Type and requirement

`boolean | string | number | array` · required

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "settings": {
    "example": {
      "value": true
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
