# Setting `general.usernameMode`

Chooses automatic player-name detection or a manual name.

## What it changes

Chooses automatic player-name detection or a manual name.

## Default

`"auto"`





## Type and allowed values

Type: `string`.

- values: `"auto"`, `"manual"`

## Example

This is a partial config snippet. Keep the other entries in your file.

```json
{
  "general": {
    "usernameMode": "auto"
  }
}
```

## Configuration location

`shroudforge/config/modloader-config.json`, JSON path `/general/usernameMode`. This configures the loader and is not copied into a mod package.

Source: `src/loader/package/src/config/loader.schema.json` and `loader.default.json`.
