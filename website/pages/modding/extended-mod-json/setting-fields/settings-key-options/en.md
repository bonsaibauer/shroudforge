# The `settings.<key>.options` field

Allowed choices and the labels players see.

## Type and requirement

`object` · optional

## Allowed values and limits

No additional values or limits are declared by the schema.

## Example

```json
{
  "settings": {
    "example": {
      "value": "easy",
      "control": "select",
      "options": {
        "easy": "Easy",
        "hard": "Hard"
      }
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
