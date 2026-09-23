# ShroudForge Parser

## Purpose

This directory contains the parser boundary used by ShroudForge. It converts Enshrouded game data into a stable snapshot that the compatibility and API crates can consume.

## Current structure

| Path | Responsibility | Status |
| --- | --- | --- |
| `core/` | ShroudForge snapshot model, transactions, and parser adapter | Active workspace crate |
| `kfc-parser/` | Unmodified Brabb3l KFC parsing and EML toolchain | Pinned Git submodule |

The first-party crate is named `shroudforge-parser`. It wraps the imported parser rather than exposing every legacy tool directly to the rest of the workspace.

## Development

Run the ShroudForge parser tests from the repository root.

```powershell
cargo test -p shroudforge-parser
```

The upstream `kfc-parser` workspace is excluded from the root Cargo workspace. Its source is unchanged from `https://github.com/Brabb3l/kfc-parser`, pinned to commit `f201f7667ddd5d0d78f0a27368f2e7481b870618` by the Git submodule entry. Keep ShroudForge-specific code in `core/` and documentation outside the submodule.

Initialize dependencies after cloning:

```powershell
git submodule update --init --recursive
```

Run upstream checks explicitly when needed:

```powershell
Set-Location Shroudforge_Parser/kfc-parser
cargo test --workspace
```
