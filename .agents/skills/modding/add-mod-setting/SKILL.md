---
name: add-mod-setting
description: Declare and wire a setting in an existing Lua mod's extended.mod.json, including its Lua consumer, grouping, localization, and live or restart behavior. Use for package settings rather than extending the global setting schema.
---

# Add a Mod Setting

Paths are relative to the repository root. Read `.agents/skills/repository/repository-working-rules.md`, the owning mod's complete manifests and Lua consumers, and `modules/modloader/manifest/extended.mod.schema.json`.

## Declare and Consume the Value

- Add the setting under `settings` in `mods/<mod-id>/extended.mod.json`. Keep its key stable and identical in Lua, group references, conditions, and locale catalogs.
- Follow `modules/modloader/read_mod_settings.rs` and `validate_mod_settings.rs` for supported value/control combinations, bounds, options, and dependency conditions. Arrays require options. Do not invent a control or add an alternate settings file.
- Choose `applyAt` based on the actual consumer. Its current default is restart. `restartRequired` must agree when present. Mark asset preparation only when that setting changes the existing preparation flow.
- Read the value through `shroudforge.settings.get(key, fallback)` with a matching fallback type. For live behavior, read it inside the relevant callback instead of retaining a load-time copy.
- Inspect `modules/mod-runtime/lua/shroudforge/v1/set_mod_settings.rs` and `modules/mod-runtime/ingame/refresh_runtime_configuration.rs` when changing propagation behavior. Keep per-mod Lua registry isolation and the existing live/restart scheduling.
- For a key binding use `control: "keybind"` and the canonical SDL/mouse binding format owned by `modules/hotkeys/`. Use the declared-setting Lua input queries and the existing conflict checks.
- Add requested group membership and package-local translations. Preserve existing values and activation when updating a package. Updates currently have no automatic setting migration.

## Preserve the Canonical Writer

The Modloader UI uses `modules/modloader/ui/save-mod-settings.ts`, host `ui/host/apply_mod_settings.rs`, and `save_mod_settings.rs`. The canonical writer is `update_mod_configuration.rs`.

Retain revision checks, unknown-key rejection, validation, keybind conflict validation, and result revisions. `ensure_eml_extension.rs` creates a missing EML compatibility extension on its first edit. Existing ShroudForge extensions are read directly and retain their provenance. Adding an ordinary package setting needs no new host command or settings UI implementation.

## Verify

Validate both the document shape and parser relationships, then trace the setting to its actual Lua effect and application timing. Use the existing focused package checks when tests are requested. The Cargo package is `shroudforge-package`. Build the frontend only if its source changed.

Report the mod/key, default and type, control, application timing, Lua consumer, grouping/locales, and checks actually performed.
