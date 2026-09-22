# kfc

## Purpose

`kfc` is the feature-gated facade for the imported Enshrouded parser libraries. It always re-exports `kfc-base` and can expose resource and content support through stable module paths.

## Features

| Feature | Default | Adds |
| --- | --- | --- |
| `resource` | Yes | `kfc-resource` as `kfc::resource` |
| `content` | No | `kfc-content` as `kfc::content` |

## Development

```powershell
cargo test -p kfc --all-features
```
