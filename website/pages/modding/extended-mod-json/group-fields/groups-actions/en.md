# The `groups[].actions` field

Action buttons whose IDs call Lua callbacks.

## Type and requirement

`array` · optional

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [],
      "actions": [
        {
          "id": "reset",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
