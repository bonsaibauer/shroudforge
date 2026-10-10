# ShroudForge API

## Purpose

This crate provides the Lua API used by ShroudForge mods. It combines parsed game data, the verified compatibility contract, settings, logging, asset access, and runtime ECS operations in one sandboxed mod environment. Mods declare the access they need in `mod.json`. Client or server targets can be selected in `extended.mod.json`. The loader checks those declarations and the current execution phase before it makes protected operations available. Calling an API function does not grant a missing capability.

## Current status

Mod authors should follow the shared [logging rules](../../../docs/sf/logging.md) for level selection, message content, and the difference between saved log levels and Debug Console display filters.

The API is part of the `1.0.0` workspace and supports two execution phases.

- Asset API users can inspect, export, and modify supported game resources during startup preparation.
- Runtime API users can query and update supported ECS components while Enshrouded is running. Runtime-only packages support live lifecycle and settings changes.
- Packages that use startup asset APIs require the next game start for those changes to take effect.
- Runtime mods should call `shroudforge.settings.get` from lifecycle callbacks instead of caching a value during module initialization when that setting must change live.
- Capability checks restrict sensitive operations such as asset writes and exports.
- Lua definition files under `definitions/` provide editor and documentation metadata.

The available game types depend on the compatibility profile selected for the installed Enshrouded build.

## Main areas

| Path | Responsibility |
| --- | --- |
| `src/env/` | Lua globals such as `game`, `runtime`, and `shroudforge` |
| `src/runner/` | Mod lifecycle and Lua state management |
| `src/cache/` | File state tracking for generated or exported data |
| `src/definition/` | Lua definition generation |
| `src/eml/v1/definitions/` | EML v1 Lua declarations |
| `src/shroudforge/v1/definitions/` | ShroudForge v1 Lua declarations |

## Read a setting in Lua

The key inside `extended.mod.json` is the key your mod reads. For example, the starter file defines `greeting` under `settings`. its Lua code calls:

```lua
local greeting = shroudforge.settings.get(
    "greeting",
    "Hello from the ShroudForge runtime!"
)
```

The first argument is the setting key. The second is a fallback for a missing value. Keep the fallback the same kind of value as the setting's `value`. Read settings in a runtime callback or action when the latest player choice should take effect. The loader updates the active runtime settings after a player saves a change. code that cached the value once during module startup still holds its old local value.

The Modloader saves a changed player value in `extended.mod.json`, under `settings.<key>.value`. Listing the key in `groups[].settings` places it in that group. An ungrouped setting still appears under the default Settings group. The [website guide](https://bonsaibauer.github.io/shroudforge/en/#doc-setting-controls) explains the fields, Lua lookup, and preview.

## Publish a Modloader notice

A runtime mod can publish a message in the player's local Modloader news feed with `shroudforge.notifications.publish`. The function writes the notice when the mod calls it. The timing is controlled by the mod code. ShroudForge does not schedule or broadcast these notices, and this API currently has no rate limit or expiry.

```lua
shroudforge.notifications.publish({
    id = "new-map-markers",
    level = "update",
    title = "New map markers for {username}",
    message = "Hey {username}, the latest version adds markers for discovered Ember Shrines. Open the mod settings to choose which marker types appear.",
    action_url = "https://example.com/my-mod/guide"
})
```

Use a stable ID for a notice that should be updated in place. The ID is scoped to the publishing mod, so two mods can use the same ID without colliding. A new ID creates a separate notice. Titles can be at most 120 bytes and messages at most 2,000 bytes. Levels are `info`, `success`, `warning`, `error`, and `update`. `action_url` is optional and must use HTTPS. Notices are saved locally and remain in the feed after they are read. Marking one as read removes it from the unread view, not from the full news list.

Each call writes to the event file for that mod and ID. Calling again with the same ID replaces its title and message instead of adding another row. A different ID creates another row that stays in the local feed. Read state is keyed by the notice ID, so replacing a notice that was already read does not make it unread again. Choose when to publish deliberately. Do not publish from a per-frame callback or create a fresh ID every time the game starts. A stable ID works for a persistent help notice. A versioned ID works when each release should have its own notice. Read version notices remain in the full feed after they are marked read.

You can use `{username}` in the title or message when addressing the player. Settings → General → Username defaults to Automatic and shows the detected name separately from the effective name. Automatic mode reads the most recently played character from the local rolling `characters` save; when the save cannot be read or its character data is ambiguous, it uses the editable fallback, which defaults to `Flameborn`. Manual mode always uses that fallback. This substitution happens on the player's device and does not change the saved mod notice.

## Network channels

`runtime.network` provides a loader-owned Steam Networking Messages transport.
Steam callback delivery remains owned by Enshrouded. the loader registers its
session-request handler but never calls Steam's callback pump. The Dedicated
Server sends an automatic health probe to authenticated connected players, and
clients learn the server SteamID64 from that exchange. Mods can use
`send_mod`/`receive_mod` for independent mod channels or `send`/`receive` for
their own raw channel. Channel `65535` is reserved for Network health checks.
public Lua channels are `0` through `65534`.

The loader's Settings > Modules > Network page stores the optional remote
server fallback and server-side client allowlist. An empty allowlist means the
server's currently authenticated Enshrouded players. The Dedicated Server
accepts and exposes messages only from that live authenticated set, intersected
with the optional allowlist. Steam identifies the P2P peer. each mod still
needs to validate its message format and operation permissions.
## Development

Run the crate checks from the repository root.

```powershell
cargo test -p shroudforge-api
```

Public API changes update the Rust implementation and Lua declarations together. EML v1 remains the current EML contract. ShroudForge ships only its current API contract. breaking changes require updating the affected mods. The website index is generated from the active declarations.
