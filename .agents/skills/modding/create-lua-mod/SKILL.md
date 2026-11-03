---
name: create-lua-mod
description: Create a ShroudForge Lua mod package directly under mods with a valid manifest, settings, optional locale/action declarations, and the appropriate startup or runtime entrypoint.
---

# Create a Lua mod package

Read `.agents/skills/repository/repository-working-rules.md` and the current package/schema owners. Preserve the user's intended behavior and choose one existing package with the same execution model as a reference. Do not copy stale schema or editor paths blindly.

## Establish the package contract

Mods live directly at `mods/<mod-id>/`, without extra category directories. Read `modules/modloader/read_mod_manifest.rs`, the manifest/extension schemas registered by that module, `read_mod_locale_catalogs.rs`, and the matching existing package. Follow actual package installation/build discovery before creating files.

The primary `mod.json` declares identity, supported API, entrypoint, capabilities, and dependencies. `extended.mod.json` supplies the existing extension fields for activation, targets, settings, groups, and actions. Reuse the current schema. Ordinary mod-specific settings do not require extending the package format. Installed packages are unpacked folders. Archive extraction belongs to installation, not runtime reads.

## Implement the requested execution model

1. Create a concrete package ID, descriptive source files, and the manifest entrypoint. Existing bundled entrypoints are `src/mod.lua`. Add `.luarc.json` only with paths matching current canonical/generated declarations.
2. Declare only required capabilities and client/server targets. Use the existing dependency and setting-enablement contract. Preserve settings types, constraints, group membership, and `applyAt`/restart semantics.
3. For startup asset edits, use the existing pregame APIs and changed-resource assignments. `mods/sf-unlock-blueprints` is startup-only. `mods/sf-crafting-essentials` demonstrates explicit pregame/ingame branching for a hybrid package. Do not run a runtime-only mod during pregame merely because it can export files.
4. For runtime behavior, return supported lifecycle callbacks and an appropriate update interval. Use checked public APIs and `runtime.overrides` for reversible native inputs. Follow `add-reversible-runtime-mod-behavior` when the feature changes live state.
5. If the package has translations, follow the current locale reader/schema and fallback rules. Keep package locale authoring separate from changing locale/schema infrastructure. Register UI actions through the existing `shroudforge.ui.on_action` path and declared controls when needed.

Do not write mod settings directly from a UI handler. The canonical Modloader revision-aware configuration writer owns saving and activation. EML compatibility extension creation remains owned by `ensure_eml_extension.rs` and is called only when the extension is missing for an EML package.

## Verify

Use the package's current schema/reader checks and `cargo test -p shroudforge-api --test bundled_lua`, whose walker compiles bundled Lua. Add meaningful lifecycle fixtures for behavior that needs them. Rebuild/package with existing `dev/build/` actions if required. For runtime/native functionality, perform the user's requested client/server smoke check with current logs and returned effects. Report package paths, execution phase/targets, defaults, dependencies/capabilities, checks, and remaining live uncertainty.
