# Runtime, client, and server

The Lua runtime runs `src/mod.lua` in the process selected by `targets`. Client means the player's game process, server means the Dedicated Server process. See [targets](#doc-targets).

## A process target is not multiplayer synchronization

`targets` selects where code runs. It does not send values to other clients or replicate a world edit. Network behavior needs suitable APIs and explicit server-side handling.

## Runtime callbacks and native support

A mod declares required runtime capabilities in `mod.json`. Some game operations use KFC Runtime, ShroudForge's built-in native module. These calls need a matching client or server build and a supported profile. A successful Lua start does not confirm that every native action is available.

Start with [Lua and KFC Runtime](#runtime), then search for specific symbols in the [API reference](#api).
