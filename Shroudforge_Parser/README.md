# ShroudForge Parser

## Purpose

This directory contains the parser boundary used by ShroudForge. It converts Enshrouded game data into a stable snapshot that the compatibility and API crates can consume.

## Current structure

| Path | Responsibility | Status |
| --- | --- | --- |
| `core/` | ShroudForge snapshot model, transactions, and parser adapter | Active workspace crate |
| `kfc-parser/` | Imported Enshrouded KFC parsing and legacy EML toolchain | Maintained compatibility subtree |

The first-party crate is named `shroudforge-parser`. It wraps the imported parser rather than exposing every legacy tool directly to the rest of the workspace.

## Development

Run the ShroudForge parser tests from the repository root.

```powershell
cargo test -p shroudforge-parser
```

The imported `kfc-parser` workspace is excluded from the root Cargo workspace. Run its commands from `Shroudforge_Parser/kfc-parser` when changing that subtree.

```powershell
Set-Location Shroudforge_Parser/kfc-parser
cargo test --workspace
```
