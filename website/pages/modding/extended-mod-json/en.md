# extended.mod.json

`extended.mod.json` extends the mod information card. Use it when your mod needs player-facing values in Modloader or needs to save its enabled state. Each field has its own page in this navigation.

## The essentials

- `schemaVersion` is currently `1`.
- `enabled` stores whether the mod is switched on.
- `targets` selects the process that loads the mod. It does not replicate changes over the network.
- `settings` stores mod-specific values.
- `groups` organizes values in the Modloader. Ungrouped settings also appear, under a default group.
- `launcher` preserves the origin of an imported EML mod.

## Example

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json",
  "schemaVersion": 1,
  "enabled": false,
  "targets": ["client"],
  "settings": {
    "greeting": {
      "value": "Hello from my mod",
      "label": "Greeting",
      "control": "text"
    }
  }
}
```

The mod setting `greeting` is read in Lua by using that exact key. See [Settings and controls](#doc-setting-controls).

## Source of the rules

[extended.mod.schema.json](../../../schemas/extended.mod.schema.json) defines the structure and data types. The package reader also checks that groups point to existing settings and choice values are valid.

[Try the live mod settings builder](#manifests).
