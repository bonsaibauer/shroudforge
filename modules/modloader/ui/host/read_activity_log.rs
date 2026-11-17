use super::ui_snapshot_types::Activity;
use std::path::{Path, PathBuf};

pub(crate) fn activity_log_path(root: &Path, server_target: bool) -> PathBuf {
    if server_target {
        shroudforge_loader_config::paths::remote_server_loader_log(root)
    } else {
        shroudforge_loader_config::paths::current_log(root)
    }
}

pub(crate) fn read_activity(root: &Path, server_target: bool) -> Result<Vec<Activity>, String> {
    shroudforge_logging::read_activity_from_path(&activity_log_path(root, server_target))
}

pub(crate) fn selected_server_target(root: &Path) -> bool {
    shroudforge_loader_config::config::read_loader(root)
        .ok()
        .and_then(|config| {
            config
                .pointer("/modules/modloaderUi/remoteTarget")
                .and_then(serde_json::Value::as_str)
                .map(|target| target == "server")
        })
        .unwrap_or(false)
}

// Log check state transitions once instead of repeating the same status on every refresh.
