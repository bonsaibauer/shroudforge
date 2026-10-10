# Setting `general.messageNameFallback`

Name used by notices when automatic detection cannot find one player name.

## What it changes

Name used by notices when automatic detection cannot find one player name.

## Default

`"Flameborn"`





## Type and allowed values

Type: `string`.

- minimum length: `1`
- maximum length: `40`

## Example

This is a partial config snippet. Keep the other entries in your file.

```json
{
  "general": {
    "messageNameFallback": "Flameborn"
  }
}
```

## Configuration location

`shroudforge/config/modloader-config.json`, JSON path `/general/messageNameFallback`. This configures the loader and is not copied into a mod package.

Source: `src/loader/package/src/config/loader.schema.json` and `loader.default.json`.
