//! SDL-backed keyboard input for ShroudForge-owned shortcuts.
//!
//! Settings, Compatibility and Lua share SDL keyboard and mouse bindings. This module owns no
//! action registry or configuration files. Callers retain their focus policy.
//! The injected runtime receives SDL input from the existing executable's
//! isolated keyboard process, preserving the game's Raw Input registration.
//! Native UI window toggles use the shared debounce gate. Mod keybind queries
//! continue to consume the raw press revisions.

mod observe_keyboard;
mod parse_hotkey;
mod window_toggle_debounce;

pub use observe_keyboard::{
    hotkey_press_revision, initialize, initialize_runtime, is_capture_active, is_hotkey_down,
    layout_revision, legacy_virtual_key_binding, run_input_process, set_capture_active,
    take_captured_hotkey,
};
pub use parse_hotkey::{Hotkey, InputDevice, parse_hotkey, parse_value};
pub use window_toggle_debounce::WindowToggleDebouncer;
