---
{
  "id": "preference-general-usernamemode",
  "locale": "en",
  "title": "Setting `general.usernameMode`",
  "summary": "Chooses automatic player-name detection or a manual name.",
  "navigation": {
    "path": [
      "Modloader settings",
      "General settings"
    ]
  }
}
---

# Setting `general.usernameMode`

Chooses automatic player-name detection or a manual name.

## What it changes

`"auto"` uses the cached character name from the existing runtime. The native menu callback reads it on the first valid character and again when the selection or name descriptor changes. The name remains cached while playing. Failed capture uses `general.messageNameFallback`. A name from a previous game process is not restored.

`"manual"` always uses `general.messageNameFallback`, which defaults to `Flameborn`.

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

Source: `modules/loader-config/loader.schema.json` and `loader.default.json`.
