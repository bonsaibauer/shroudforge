# Setting `paths.runtime`

Folder for Lua and native runtime data and detected game profiles.

## What it changes

Folder for Lua and native runtime data and detected game profiles.

## Default

Resolved automatically from the game folder

An unset value is resolved relative to the game folder at startup. A custom relative path also starts from the game folder and is then saved as an absolute path. Absolute paths are preserved.



## Type and allowed values

Type: `string | null`.

- minimum length: `1`

## Example

This is a partial config snippet. Keep the other entries in your file.

```json
{
  "paths": {
    "runtime": "./custom-runtime"
  }
}
```

## Configuration location

`shroudforge/config/modloader-config.json`, JSON path `/paths/runtime`. This configures the loader and is not copied into a mod package.

Source: `src/loader/package/src/config/loader.schema.json` and `loader.default.json`.
