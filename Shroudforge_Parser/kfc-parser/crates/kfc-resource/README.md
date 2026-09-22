# kfc-resource

## Purpose

`kfc-resource` builds on `kfc-base` to decode and encode structured Enshrouded resource values stored in KFC files. It maps reflected binary fields to Rust values that can be serialized through Serde.

## Current status

This is an active parser library in the imported `kfc-parser` workspace. It covers resource values and mapped objects. Media content such as images and audio belongs to `kfc-content`.

## Development

```powershell
cargo test -p kfc-resource
```
