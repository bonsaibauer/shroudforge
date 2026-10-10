# Setting `logging.minimumLevel`

Lowest level ShroudForge writes to log files. The Debug Console also has its own display filter.

## What it changes

Lowest level ShroudForge writes to log files. The Debug Console also has its own display filter.

## Default

`"INFO"`





## Type and allowed values

Type: `string`.

- values: `"TRACE"`, `"DEBUG"`, `"INFO"`, `"WARN"`, `"ERROR"`

## Example

This is a partial config snippet. Keep the other entries in your file.

```json
{
  "logging": {
    "minimumLevel": "INFO"
  }
}
```

## Configuration location

`shroudforge/config/modloader-config.json`, JSON path `/logging/minimumLevel`. This configures the loader and is not copied into a mod package.

Source: `src/loader/package/src/config/loader.schema.json` and `loader.default.json`.
