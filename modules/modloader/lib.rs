mod path_types;
use path_types as alias;
mod mod_errors;
pub use mod_errors::*;
mod mod_log_macros;
use mod_log_macros as log;
mod check_hotkey_conflicts;
pub mod check_mod_conflicts;
pub use check_hotkey_conflicts::{
    HotkeyAssessment, HotkeyAssessmentTarget, HotkeyConflict, ProtectedHotkey,
    ProtectedHotkeyGroups, assess_hotkey_change, find_hotkey_conflicts, protected_hotkeys,
    validate_hotkey_configuration,
};
pub use validate_mod_settings::setting_is_enabled;
pub mod dll_manager;
mod infer_mod_api_contract;
pub mod mod_profiles;
pub use check_mod_conflicts as compatibility;
mod plan_mod_load_order;
pub use plan_mod_load_order::*;
mod mod_registry;
pub use mod_registry::{Mod, ModRegistry};
mod find_installed_mods;
mod validate_mod_manifest;
mod validate_setting_presets;
pub use validate_mod_manifest::validate_manifest;
mod mod_package_filesystem;
mod read_input_reservations;
mod validate_input_reservations;
pub use mod_package_filesystem::FileSystem;
mod mod_manifest_types;
pub use mod_manifest_types::*;
mod read_mod_manifest;
pub use read_mod_manifest::*;
pub(crate) mod read_mod_locale_catalogs;
mod resolve_mod_locale;
pub use resolve_mod_locale::*;
mod parse_mod_manifest;
pub use parse_mod_manifest::*;
mod calculate_manifest_revision;
pub use calculate_manifest_revision::*;
pub use infer_mod_api_contract::infer_api_contract;
mod calculate_mod_config_revision;
mod queue_mod_action;
mod read_mod_config_revision;
mod read_mod_settings;
mod save_key_reservation;
mod save_mod_settings;
mod set_mod_enabled;
mod update_mod_configuration;
mod validate_mod_settings;
pub use calculate_mod_config_revision::*;
pub use queue_mod_action::*;
pub use read_mod_config_revision::*;
pub use save_key_reservation::*;
pub use save_mod_settings::*;
pub use set_mod_enabled::*;
pub mod calculate_preparation_fingerprint;
pub use calculate_preparation_fingerprint as prepared;
pub mod documents;
pub mod ensure_eml_extension;
pub mod package_files;
pub mod package_lookup;
pub mod config {
    pub use crate::validate_mod_settings::validate_settings;
    pub use crate::{
        infer_api_contract, manifest_revision, parse_manifest, parse_manifest_with_extension,
        read_manifest, read_manifest_path, valid_id,
    };
    pub use shroudforge_loader_config::config::*;
}
pub mod compatibility_status;
mod installed_version;
pub use installed_version::installed_version;
pub mod multiplayer_compatibility;
pub use multiplayer_compatibility::*;
pub use shroudforge_loader_config::paths;

mod create_mod;
pub use create_mod::create_mod;
