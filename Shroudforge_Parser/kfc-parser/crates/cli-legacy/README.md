# cli-legacy

## Purpose

`cli-legacy` is the deprecated command-line application for unpacking and repacking Enshrouded KFC data, restoring backups, extracting reflection types, and converting Impact programs.

## Current status

The application is retained for maintenance and compatibility. It may receive fixes, but new ShroudForge workflows should use the root workspace tools and generated API site.

## Commands

| Command | Purpose |
| --- | --- |
| `unpack` | Export matching resources to a directory or standard output |
| `repack` | Import qualified resource files and create a KFC backup |
| `restore` | Restore the original KFC file from its backup |
| `extract-types` | Extract reflected types from the game executable |
| `impact disassemble` | Convert an Impact descriptor to readable program files |
| `impact assemble` | Build an Impact descriptor from program files |
| `impact extract-nodes` | Export known Impact nodes from reflection data |

Use `--help` on the application or a subcommand for the current arguments.

```powershell
cargo run -p cli-legacy -- --help
cargo run -p cli-legacy -- unpack --help
```

Unpacking and repacking require an Enshrouded game directory. Impact assembly expects a shared base name for `.impact`, `.shutdown.impact`, and `.data.json` inputs.
