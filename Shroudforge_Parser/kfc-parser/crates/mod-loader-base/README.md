# mod-loader-base

## Purpose

`mod-loader-base` provides the shared package registry, configuration, environment, alias, and logging types used by the legacy EML loader crates.

## Current status

This is compatibility code for the imported EML toolchain. It validates and prepares legacy mod packages but does not execute Lua or inject runtime behavior by itself.

## Development

```powershell
cargo test -p mod-loader-base
```
