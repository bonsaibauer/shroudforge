# ShroudForge API Site

## Purpose

This directory builds the static API reference published for ShroudForge mod authors. It combines the public Lua API model with sanitized parser snapshots for a supported Enshrouded build.

## Current inputs

| Path | Responsibility |
| --- | --- |
| `data/api.json` | Public ShroudForge API model |
| `data/current.json` | Pointer to the current game snapshot |
| `data/<snapshot>/` | Types, resources, profile metadata, and generated runtime reference |
| `tools/` | Build, sanitization, runtime reference, and validation scripts |
| `index.html`, `app.js`, `styles.css` | Static site application |

The checked-in snapshot currently describes Enshrouded build `1076226`.

## Build and validation

Run the complete pipeline from the repository root.

```powershell
npm run site:check
```

The command rebuilds the site data, removes unsafe snapshot content, regenerates the runtime ECS reference, and validates links and data contracts.

Do not edit generated snapshot outputs by hand. Change their source or generator and rebuild them instead.
