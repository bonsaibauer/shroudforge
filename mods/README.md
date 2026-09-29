# Bundled mod packages

The release build checks each direct child folder for a `mod.json` file with a
mod ID and the Lua entry point at `src/mod.lua`. It also checks the Lua files
before packaging them. The repository can contain work folders without these
package files; those folders are not copied into the release.

## One rule for each manifest

- `mod.json` is the package manifest. It contains the package identity, version,
  author, dependencies, and explicit `capabilities`. Optional `license` and
  `icon` fields accept a string or `null`; `null` means no value is declared.
  Omitting either field is also valid. An icon is a root filename or a path under
  `assets/`.
- `capabilities` lists only the permissions the Lua code uses: `patch` for EML
  asset writes, `export` for `io.export` and export reads, and `runtime` for
  ShroudForge in-game runtime APIs. EML mods that register a package DLL also
  use `runtime-register-dll`. API scanning derives execution phase and process
  scope; it never adds permissions that the manifest omitted.
- `extended.mod.json` is optional ShroudForge state and metadata. It holds the
  enabled value, setting values, groups, actions, links, and changelog. The
  loader writes player changes into this same file. Package updates can reset
  those choices; there is no settings migration.
- An EML package without ShroudForge-specific state can omit
  `extended.mod.json`. The loader creates it when a ShroudForge value is first
  changed. EML `mod.json` stays in the EML format.

The SF gameplay packages use the same two-file format. EML data tools only need
`mod.json` until a ShroudForge setting or action is added.

## Capability assignment

| Packages | Capabilities | Reason |
| --- | --- | --- |
| `fishing-exporter`, `item-translator`, `kfc-parser-mimic` | `export` | Write generated reports through `io.export`; asset reads do not need `patch`. |
| `sf-infinite-item-split`, `sf-infinite-item-use`, `sf-no-fall-damage`, `sf-no-resource-cost`, `sf-no-stamina-loss`, `sf-unlimited-flight` | `runtime` | Use approved in-game runtime patch operations. |
| `sf-unlock-blueprints` | `patch` | Write EML game assets during startup preparation. |
| `world-editor` | `runtime`, `export` | Standalone Lua world editor using the public runtime API and export storage. |

Inline `shroudforge` data is not part of the current manifest contract and is
rejected. ShroudForge state belongs in the neighboring `extended.mod.json`.

## Package layout

```text
mod-name/
├── mod.json
├── extended.mod.json   # only when ShroudForge-specific metadata is needed
└── src/
    └── mod.lua
```

## API details

`runtime.patch.*` names approved native runtime operations. It belongs to the
`runtime` capability; it does not mean EML asset-writing `patch`. Asset writes
through `game.assets.write` need the `patch` capability. Keep Lua entrypoints
free of gameplay reads and writes at module initialization; register actions
there, then do runtime work from lifecycle or action callbacks.

For a setting that Lua can read, declare its key, starting `value`, and optional display details in `extended.mod.json`. List the key in one group's `settings` array so the Modloader shows it. Read the current value in Lua with `shroudforge.settings.get("key", fallback)`; use the same key and value type in both places. The [package guide](../docs/sf/mod-packages.md) has a complete JSON/Lua example, and [`templates/mod/`](../templates/mod/) is a working version.
Build-specific signatures and trampoline data belong in KFC compatibility
profiles. Lua code uses stable operation names.
