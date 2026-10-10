# Publish Modloader notices

`shroudforge.notifications.publish` adds a mod-authored notice to the local Modloader feed. It is not sent to other players. Call it deliberately, for example after a meaningful change or a completed step.

## Example

```lua
shroudforge.notifications.publish({
  id = "world-ready",
  title = "World is ready",
  message = "Your world data has been checked.",
  level = "info"
})
```

The exact payload and accepted levels are in the [API reference](#api). The mod needs the matching runtime feature and `runtime` in `mod.json`.

## IDs and repeated notices

The ID is stable within the mod's namespace. Reusing it updates the existing notice, while a new ID creates another entry. The feed is stored locally in the configured state location. The function does not schedule, repeat, or distribute notices automatically.

## When to publish

Report confirmed events. Do not call it per frame or in a tight loop. Use [mod logging](#doc-api-logging) for technical diagnostics. The stored format is described in [modloader-content.md](https://github.com/bonsaibauer/shroudforge/blob/main/docs/sf/modloader-content.md).
