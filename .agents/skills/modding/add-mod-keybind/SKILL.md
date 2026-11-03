---
name: add-mod-keybind
description: Add a declared keyboard or mouse shortcut to a Lua mod and connect its action through the existing ShroudForge input queries, settings editor, and conflict checks.
---

# Add a Mod Key Binding

Paths are relative to the repository root. Read `.agents/skills/repository/repository-working-rules.md`, the target mod, `modules/hotkeys/parse_hotkey.rs`, and the input bindings in `modules/mod-runtime/lua/shroudforge/v1/create_mod_api.rs`.

Declare the key in the mod's `extended.mod.json` as a setting with `control: "keybind"` and a string value. Match its key, group membership, labels, and package locale entries. Choose application timing consistent with the actual callback and the existing runtime settings refresh.

Use the canonical string format, such as `Ctrl+SDL:29|Mouse:4`. A binding has at most one keyboard chord and one mouse chord, in that order. Modifier order is Ctrl, Alt, Shift, Meta. Empty disables the shortcut. SDL supplies key validity and localized labels. Do not introduce browser keycodes or a separate input hook.

Connect the requested action with `shroudforge.input.is_keybind_down(settingKey)`, `was_keybind_pressed(settingKey)`, or `keybind_label(settingKey)` as appropriate. Declare the existing `runtime` capability and keep the mod's target/lifecycle checks. Do not use the removed F1–F8 `is_key_down` API.

The shared editor is `modules/hotkeys/ui/edit-keybind.tsx`. `ui/show-hotkeys.tsx` discovers declared bindings from the existing snapshot. The Modloader owns keybind assessment, protected-key policy, enabled-setting conditions, and conflicts. Preserve its normal revision-aware saving and assessment flow. An ordinary mod binding needs no new UI or host command.

The input owner is `modules/hotkeys/observe_keyboard.rs` with `sdl_keyboard.c`. Its existing isolated input process preserves game input registration. Capture suppresses shortcut dispatch across participating windows. Native window toggles use `WindowToggleDebouncer`, while mod input queries retain raw press revisions. Do not apply the native window's 250 ms debounce to mod queries.

Verify press versus hold behavior, short taps, keyboard/mouse alternatives, empty bindings, setting changes, active/disabled mod state, and capture/conflict handling when functional testing is requested. Use the existing package and hotkey checks appropriate to the changed owner.

Report the mod, setting key, action callback, binding/default, target and timing, conflict path, and checks performed.
