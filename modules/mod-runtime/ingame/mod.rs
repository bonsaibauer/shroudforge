use crate::{ShroudForgeApi, runner::LuaModRunner};
use mlua::RegistryKey;

mod activate_runtime_mod;
mod load_native_plugins;
mod load_runtime_callbacks;
mod plan_runtime_mods;
mod process_runtime_reload_request;
mod publish_runtime_status;
mod refresh_runtime_configuration;
mod reload_runtime_packages;
mod restore_runtime_packages;
mod run_diagnostic_job;
mod start_ingame_runtime;
mod stop_ingame_runtime;
mod unload_runtime_mod;
mod unload_runtime_packages;
mod update_ingame_runtime;
mod update_mod_callbacks;
mod watch_package_revisions;

#[cfg(test)]
use refresh_runtime_configuration::settings_only_manifest_change;
#[cfg(test)]
mod lifecycle_tests;

pub(crate) struct Lifecycle {
    pub(crate) id: String,
    pub(crate) load: Option<RegistryKey>,
    pub(crate) update: Option<RegistryKey>,
    pub(crate) unload: Option<RegistryKey>,
    pub(crate) update_interval: std::time::Duration,
    pub(crate) next_update: std::time::Instant,
    pub(crate) active: bool,
    pub(crate) cleanup_pending: bool,
}

pub struct IngameRuntime {
    diagnostics: shroudforge_runtime_diagnostics::Session,
    root: std::path::PathBuf,
    api: Option<ShroudForgeApi>,
    file_name: String,
    loaded: serde_json::Map<String, serde_json::Value>,
    errors: serde_json::Map<String, serde_json::Value>,
    next_status: std::time::Instant,
    configurations: Vec<(String, std::path::PathBuf, serde_json::Value)>,
    observed: std::collections::HashMap<std::path::PathBuf, String>,
    package_revisions: watch_package_revisions::PackageRevisions,
    runner: LuaModRunner,
    lifecycle: Vec<Lifecycle>,
    processed_reload_request: Option<String>,
    publish_on_drop: bool,
}

impl IngameRuntime {
    pub fn active_mod_ids(&self) -> Vec<String> {
        self.lifecycle
            .iter()
            .filter(|item| item.active && !item.cleanup_pending)
            .map(|item| item.id.clone())
            .collect()
    }
}
