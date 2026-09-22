# mod-manager-cli

## Purpose

`mod-manager-cli` is the command-line manager for the imported legacy EML package system.

## Current status

The application can create a mod through an interactive dialog, run the legacy loader in patch, export, or runtime modes, and restore an original KFC backup. It is not the current ShroudForge launcher.

## Commands

| Command | Purpose |
| --- | --- |
| `create` | Create a legacy mod package interactively |
| `run` | Prepare mods and optionally patch, export, or start runtime loading |
| `restore` | Restore the original Enshrouded files |

Use command help for the authoritative argument list.

```powershell
cargo run -p mod-manager-cli -- --help
cargo run -p mod-manager-cli -- run --help
```

For ShroudForge `1.0.0`, use `shroudforge.exe` from the repository root workspace instead.
