---
{
  "id": "api-runtime",
  "locale": "en",
  "title": "Runtime, client, and server",
  "summary": "Runtime mods start in a selected game process. Native game operations also depend on the game build and a supported profile.",
  "navigation": {
    "path": [
      "Lua API guides",
      "Feature guides"
    ]
  },
  "apiPrefixes": [
    "runtime.",
    "RuntimeNativeBinding.",
    "RuntimeFunctionModifier.",
    "RuntimeFeatureStatus."
  ]
}
---

# Runtime, client, and server

The Lua runtime runs `src/mod.lua` in the process selected by `targets`. Client means the player's game process, server means the Dedicated Server process. See [targets](#doc-targets).

## A process target is not multiplayer synchronization

`targets` selects where code runs. It does not send values to other clients or replicate a world edit. Network behavior needs suitable APIs and explicit server-side handling.

## Runtime callbacks and native support

A mod declares required runtime capabilities in `mod.json`. Some game operations use KFC Runtime, ShroudForge's built-in native module. These calls need a matching client or server build and a supported profile. A successful Lua start does not confirm that every native action is available.

Start with [Lua and KFC Runtime](#runtime), then search for specific symbols in the [API reference](#api).

For a read-only example that logs one runtime-registry snapshot, copy [`registry-overview.lua`](https://github.com/bonsaibauer/shroudforge/blob/HEAD/docs/examples/runtime/registry-overview.lua) to `src/mod.lua` in a test mod that declares the `runtime` capability.

The [Modding](#doc-modding-guide) category explains the [main-menu state](#doc-modding-main-menu) and [local character name](#doc-modding-player-name) queries with prerequisites, return values, and complete Lua examples.
