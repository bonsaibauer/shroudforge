# Setting `general.locale`

Language of the Modloader interface, as a valid language tag.

## What it changes

Language of the Modloader interface, as a valid language tag.

## Default

`"de"`





## Type and allowed values

Type: `string`.

- pattern: `^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$`

## Example

This is a partial config snippet. Keep the other entries in your file.

```json
{
  "general": {
    "locale": "de"
  }
}
```

## Configuration location

`shroudforge/config/modloader-config.json`, JSON path `/general/locale`. This configures the loader and is not copied into a mod package.

Source: `src/loader/package/src/config/loader.schema.json` and `loader.default.json`.
