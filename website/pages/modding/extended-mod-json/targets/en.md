# `targets`, choose client and server

`targets` is a list in `extended.mod.json`. It selects the process in which the mod is loaded.

| Value | Where the mod runs |
| --- | --- |
| `["client"]` | In the player's local Enshrouded game process. |
| `["server"]` | In the Dedicated Server process. |
| `["client", "server"]` | In both the client and Dedicated Server. |

## Examples

Client only:

```json
{
  "targets": ["client"]
}
```

Server only:

```json
{
  "targets": ["server"]
}
```

Both processes:

```json
{
  "targets": ["client", "server"]
}
```

## What it does not do

A client mod is not automatically synchronized with the server. `targets` does not replicate Lua state or world edits, and it does not send a Steam P2P request. Those need suitable network and server-side logic.

## Defaults and EML

New ShroudForge packages should set `["client"]` explicitly. An EML package without explicit `targets` runs on client and server, including an extension file with `launcher: "EML"`. Set `targets` explicitly when migrating or when you need another target. See [EML migration](#doc-eml-migration).

For multiplayer, install the mod wherever its code needs to run. See the [multiplayer quickstart](#server).
