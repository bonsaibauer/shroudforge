# Setting `exports.enabled`

Enables or disables mod export functions.

## What it changes

Enables or disables mod export functions.

## Default

`true`





## Type and allowed values

Type: `boolean`.

No additional values or limits are declared by the schema.

## Example

This is a partial config snippet. Keep the other entries in your file.

```json
{
  "exports": {
    "enabled": true
  }
}
```

## Configuration location

`shroudforge/config/modloader-config.json`, JSON path `/exports/enabled`. This configures the loader and is not copied into a mod package.

Source: `src/loader/package/src/config/loader.schema.json` and `loader.default.json`.
