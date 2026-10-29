//! SDL-backed keyboard input for ShroudForge-owned shortcuts.
//!
//! Settings, Compatibility and Lua share SDL keyboard and mouse bindings. This module owns no
//! action registry or configuration files. Callers retain their focus policy.
//! The injected runtime receives SDL input from the existing executable's
//! isolated keyboard process, preserving the game's Raw Input registration.
//! Native UI window toggles use the shared debounce gate. Mod keybind queries
//! continue to consume the raw press revisions.
//! In the game process, input reservations keep some keys, mouse buttons and
//! the mouse wheel from the game while a mod reports a context; the mods
//! still read them.

mod filter_game_input;
mod observe_keyboard;
mod parse_hotkey;
mod reserve_game_input;
mod window_toggle_debounce;

pub use filter_game_input::install_input_filter;
pub use observe_keyboard::{
    hotkey_press_revision, initialize, initialize_runtime, is_capture_active, is_hotkey_down,
    layout_revision, legacy_virtual_key_binding, run_input_process, set_capture_active,
    take_captured_hotkey,
};
pub use parse_hotkey::{Hotkey, InputDevice, parse_hotkey, parse_value};
pub use reserve_game_input::{
    CONTEXT_LEASE, ReadWheel, ReservedKey, release_reserved_input, set_input_context,
    set_reserved_input, take_wheel_steps,
};
pub use window_toggle_debounce::WindowToggleDebouncer;
