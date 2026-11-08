mod document;
pub use document::{Document, Storage};
mod calculate_revision;
mod claim_control_actions;
pub mod documents;
mod initialize_loader_settings;
mod lock_installation;
mod migrate_legacy_toggle_key_settings;
#[path = "resolve_installation_paths.rs"]
pub mod paths;
mod publish_modloader_client_pid;
mod publish_window_visibility;
mod read_document;
mod read_loader_in_background;
mod read_loader_settings;
pub(crate) mod read_loader_state;
mod request_window_visibility;
mod request_world_editor_module_settings;
mod resolve_document_path;
mod save_loader_settings;
mod update_loader_state;
mod validate_document;
mod window_state;
mod write_document;
mod write_document_in_background;
mod write_json_atomically;

pub const UPDATER_CONTROL_ACTIONS: &[&str] = &[
    "checkUpdates",
    "installMod",
    "updateMod",
    "queueSystemUpdate",
    "selectUpdateQueueItems",
    "removeUpdateQueueItem",
    "clearUpdateQueue",
    "cancelUpdate",
    "startUpdateQueue",
    "stageUpdate",
    "restoreGamefiles",
    "captureGamefileOriginals",
];

pub mod config {
    pub use crate::calculate_revision::*;
    pub use crate::claim_control_actions::*;
    pub use crate::initialize_loader_settings::*;
    pub use crate::lock_installation::*;
    pub use crate::migrate_legacy_toggle_key_settings::*;
    pub use crate::publish_modloader_client_pid::*;
    pub use crate::publish_window_visibility::*;
    pub use crate::read_document::*;
    pub(crate) use crate::read_loader_settings::read_loader_unvalidated;
    pub use crate::read_loader_in_background::*;
    pub use crate::read_loader_settings::*;
    pub use crate::read_loader_state::*;
    pub use crate::request_window_visibility::*;
    pub use crate::request_world_editor_module_settings::*;
    pub use crate::resolve_document_path::*;
    pub use crate::save_loader_settings::*;
    pub use crate::update_loader_state::*;
    pub use crate::validate_document::*;
    pub use crate::window_state::*;
    pub use crate::write_document::*;
    pub use crate::write_document_in_background::*;
    pub use crate::write_json_atomically::*;
}
mod read_runtime_pid;
mod runtime_is_active;
pub mod runtime_status {
    pub use crate::read_runtime_pid::*;
    pub use crate::runtime_is_active::*;
}
