# Understand and migrate EML mods

`mod.json` is based on the EML manifest format. ShroudForge continues to use its mod ID, name, version, dependencies, and capability declarations. ShroudForge-specific values belong in `extended.mod.json`.

## Origin and launcher badge

An EML package without an extension file is treated as a legacy EML package. When ShroudForge creates an extension while saving it, it writes `launcher: "EML"` to preserve that origin. ShroudForge mods may omit `launcher`.

## Client and server

EML packages without explicit `targets` default to client and server, including packages whose extension declares `launcher: "EML"`. New ShroudForge packages default to client. Set `targets` explicitly when another process is needed. Process targets do not guarantee network replication. See [targets](#doc-targets).

## Capabilities

Check which capabilities the mod actually uses. `runtime-register-dll` refers to a DLL included in the mod package, not ShroudForge's built-in `kfc-runtime.dll`. It schedules EML DLL registration for the runtime phase.

## A cautious migration

1. Back up the mod folder.
2. Leave `mod.json` unchanged unless a verified adjustment is needed.
3. Add the optional `extended.mod.json` for ShroudForge settings.
4. Set `launcher: "EML"` and the intended `targets` explicitly.
5. Check the mod in Modloader and in the matching client or server log.

A valid package does not automatically mean every EML feature or native operation is compatible with every game build. The [searchable API reference](#api) distinguishes EML and ShroudForge functions.
