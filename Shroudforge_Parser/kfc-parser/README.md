# Enshrouded Modding Tools

## Purpose

This imported workspace contains the KFC parsing libraries and the legacy Enshrouded Mod Loader toolchain used by ShroudForge's parser integration.

## Current status

The parsing crates remain useful for reading Enshrouded container, resource, and content formats. The EML mod-loader applications are retained for compatibility and historical tooling. New ShroudForge runtime development belongs in the root workspace, not in the legacy EML applications.

## Workspace map

| Crate | Responsibility | Status |
| --- | --- | --- |
| `kfc-base` | KFC containers, reflection, GUIDs, and hashing | Parser library |
| `kfc-resource` | Structured resource values | Parser library |
| `kfc-content` | Images, audio, meshes, and Impact data | Parser library |
| `kfc` | Feature-gated parser facade | Parser library |
| `cli-legacy` | Unpack, repack, restore, and Impact commands | Deprecated CLI |
| `mod-loader-*` | Legacy EML package and runtime layers | Compatibility code |
| `mod-manager-*` | Legacy command-line and graphical managers | Legacy applications |
| `dbghelp-proxy`, `dinput8-proxy` | Windows DLL proxy entry points | Legacy bootstrap |

## Development

This workspace is excluded from the ShroudForge root workspace.

```powershell
Set-Location Shroudforge_Parser/kfc-parser
cargo test --workspace
```

The mdBook sources are under `docs/`. Existing EML documentation remains available at <https://brabb3l.github.io/kfc-parser>.

## Community

The original EML community is available on [Discord](https://discord.gg/HKKyeMsKfW).
