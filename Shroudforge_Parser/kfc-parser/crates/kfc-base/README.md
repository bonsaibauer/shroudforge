# kfc-base

## Purpose

`kfc-base` implements the low-level Enshrouded KFC format. It reads and writes containers, extracts reflection metadata from supported Windows executables, and provides the GUID and hashing primitives shared by higher-level crates.

## Current status

This is an active parser library in the imported `kfc-parser` workspace. It does not provide a command-line interface and does not interpret resource or media payloads on its own.

## Main modules

- `container` handles KFC headers, maps, readers, and writers.
- `reflection` models and extracts reflected game types.
- `guid` implements base, resource, and content identifiers.
- `hash` implements the content and FNV hash helpers used by the format.

## Development

```powershell
cargo test -p kfc-base
```
