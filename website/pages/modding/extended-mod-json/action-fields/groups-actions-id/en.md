# The `groups[].actions[].id` field

Unique action name that the mod registers in Lua.

## Type and requirement

`string` · required

## Allowed values and limits

- pattern: `^[A-Za-z0-9._-]{1,80}$`

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "actions": [
        {
          "id": "resetFeature",
          "label": "Reset"
        }
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended-mod.schema.json). The package reader checks additional relationships between fields.
