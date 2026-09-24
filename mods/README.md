# Bundled Mods

## Purpose

This directory contains the Lua mods shipped with ShroudForge releases. Every child directory is an independent mod package with a `mod.json` manifest and a `src/mod.lua` entrypoint.

## Current mods

| Mod | Phase | Target | Behavior | Modloader UI |
| --- | --- | --- | --- | --- |
| Flight | Runtime | Client | Keeps local locomotion in the flying state and can suppress fall damage | Settings and reset action |
| Infinite Item Split | Runtime | Client | Restores the amount removed from a source stack after a split | Split-type selection |
| Infinite Item Use | Runtime | Client | Restores a consumed item to its addressed inventory slot | Item ID exclusions |
| No Fall Damage | Runtime | Client | Clears the local player's typed fall-damage state | Manual reset action |
| No Resource Cost | Startup assets | Client and server | Sets selected recipe input and resource costs to zero | Item, energy, and water controls |
| No Stamina Loss | Runtime | Client | Clears depletion and optionally restores typed maximum stamina | Depletion and refill controls |
| Unlock Blueprints | Startup assets | Client and server | Replaces supported recipe knowledge requirements | Lifecycle information and restart notice |

## UI policy

Every installed mod receives one activation switch stored in that package's `mod.json`. The switch controls whether the package participates in startup asset application or runtime execution. Missing `enabled` values in third-party EML packages default to disabled; bundled ShroudForge packages explicitly set it to `true`.

Every bundled mod must also declare a useful `ui` page in its manifest. A page should expose only behavior-specific settings or safe actions. Mods must not duplicate the loader activation switch in their own settings.

## Package layout

```text
mod-name/
├── mod.json
└── src/
    └── mod.lua
```

Keep user-facing copy in `mod.json` concise and in English. Use commas or full stops instead of semicolons in prose.

## Validation

The root workspace tests parse every bundled manifest and verify capability requirements. The release build also rejects unsupported low-level tokens in bundled Lua code.

```powershell
cargo test -p shroudforge-modloader
```
