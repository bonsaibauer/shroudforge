# The `groups[].actions[].label` field

Player-facing name displayed in the Modloader.

## Type and requirement

`string` · required

## Allowed values and limits

- minimum length: `1`
- maximum length: `120`

## Example

```json
{
  "groups": [
    {
      "label": "Example",
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

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended-mod.schema.json). The package reader checks additional relationships between fields.
