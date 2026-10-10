# The `groups[].settings` field

Keys of existing mod settings displayed in this group.

## Type and requirement

`array` · optional

## Allowed values and limits

- Items must be unique.

## Example

```json
{
  "groups": [
    {
      "label": "Example",
      "settings": [
        "enabledFeature"
      ]
    }
  ]
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
