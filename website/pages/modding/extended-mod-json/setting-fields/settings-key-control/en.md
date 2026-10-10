# The `settings.<key>.control` field

Input control displayed by the Modloader.

## Type and requirement

`string` · optional

## Allowed values and limits

- values: `"toggle"`, `"checkbox"`, `"text"`, `"textarea"`, `"number"`, `"slider"`, `"select"`, `"radio"`, `"segmented"`, `"multiselect"`, `"keybind"`, `"color"`

## Example

```json
{
  "settings": {
    "example": {
      "value": 1,
      "control": "slider"
    }
  }
}
```

## Source

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). The package reader checks additional relationships between fields.
