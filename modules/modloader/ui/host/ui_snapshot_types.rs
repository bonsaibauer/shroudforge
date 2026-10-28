use super::read_ui_configuration::{default_refresh, default_toggle_key};
use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModuleConfig {
    #[serde(default = "default_toggle_key")]
    pub(crate) toggle_key: String,
    #[serde(default = "default_refresh")]
    pub(crate) refresh_milliseconds: u64,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserConfig {
    #[serde(default)]
    pub(crate) update_provider: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) locale: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModInfo {
    pub(crate) revision: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) dependencies: Vec<ModDependencyInfo>,
    pub(crate) ecosystem: &'static str,
    pub(crate) target: String,
    pub(crate) asset: bool,
    pub(crate) runtime: bool,
    pub(crate) description: Option<String>,
    pub(crate) authors: Vec<String>,
    pub(crate) license: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) links: shroudforge_package::ModLinks,
    pub(crate) source: &'static str,
    pub(crate) enabled: bool,
    pub(crate) settings: Vec<serde_json::Value>,
    pub(crate) groups: Vec<serde_json::Value>,
    pub(crate) presets: serde_json::Value,
    pub(crate) changelog: Vec<String>,
    pub(crate) assets: std::collections::HashMap<String, String>,
    pub(crate) setting_values: serde_json::Value,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModDependencyInfo {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) optional: bool,
    pub(crate) installed_version: Option<String>,
    pub(crate) status: String,
}

pub(crate) type Activity = shroudforge_logging::ActivityLogEntry;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub(crate) diagnostics: serde_json::Value,
    pub(crate) dll_candidates: Vec<shroudforge_package::dll_manager::DllCandidate>,
    pub(crate) dll_chain_entries: Vec<shroudforge_package::dll_manager::CandidateLoadEntry>,
    pub(crate) windows: serde_json::Value,
    pub(crate) configuration: serde_json::Value,
    pub(crate) connected: bool,
    pub(crate) mode: &'static str,
    pub(crate) game_version: String,
    pub(crate) version: String,
    pub(crate) mods: Vec<ModInfo>,
    pub(crate) hotkey_conflicts: Vec<shroudforge_package::HotkeyConflict>,
    pub(crate) activity: Vec<Activity>,
    pub(crate) notices: Vec<shroudforge_news::Notice>,
    pub(crate) news_templates: serde_json::Value,
    pub(crate) read_notice_ids: Vec<String>,
    pub(crate) release: serde_json::Value,
    pub(crate) release_announcement: serde_json::Value,
    pub(crate) update_queue: serde_json::Value,
    pub(crate) backup_management: serde_json::Value,
    pub(crate) settings: SnapshotSettings,
    pub(crate) catalog: serde_json::Value,
    pub(crate) locale: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotSettings {
    pub(crate) config_revision: String,
    pub(crate) module_preferences: serde_json::Value,
    pub(crate) protected_hotkeys: Vec<shroudforge_package::ProtectedHotkey>,
    pub(crate) compact_mode: bool,
    pub(crate) reduced_motion: bool,
    pub(crate) show_native_mod_menu: bool,
    pub(crate) zoom_percent: u16,
    pub(crate) username_mode: String,
    pub(crate) detected_username: Option<String>,
    pub(crate) effective_username: String,
    pub(crate) username_source: String,
    pub(crate) message_name_fallback: String,
    pub(crate) log_level: String,
    pub(crate) export_enabled: bool,
    pub(crate) runtime_profile_id: Option<String>,
    pub(crate) active_runtime_profile: Option<String>,
    pub(crate) runtime_profiles: Vec<String>,
    pub(crate) runtime_profile_directory: String,
    pub(crate) diagnostic_jobs: Vec<shroudforge_runtime_diagnostics::DiagnosticJob>,
    pub(crate) diagnostic_job_errors: Vec<String>,
    pub(crate) diagnostic_job_directory: String,
    pub(crate) diagnostic_selected_job_id: String,
    pub(crate) base_url: String,
    pub(crate) project_id: String,
    pub(crate) paths: Vec<StoragePath>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoragePath {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) default_path: String,
}
