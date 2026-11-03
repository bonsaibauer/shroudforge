//! Canonical paths for game data and mutable ShroudForge state.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const UPDATER_REPLACEMENT_GRACE_SECONDS: u64 = 120;
static UPDATER_MISSING_SINCE: OnceLock<Mutex<HashMap<PathBuf, Instant>>> = OnceLock::new();

fn configured_dir(root: &Path, key: &str) -> Option<PathBuf> {
    let value = crate::config::read_loader(root).ok()?;
    let text = value.pointer(&format!("/paths/{key}"))?.as_str()?;
    let path = PathBuf::from(text);
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

pub fn data_dir(root: &Path) -> PathBuf {
    root.join("shroudforge")
}
pub fn backups_dir(root: &Path) -> PathBuf {
    data_dir(root).join("backups")
}
pub fn recycle_bin_dir(root: &Path) -> PathBuf {
    data_dir(root).join("recycle-bin")
}
pub fn updater_jobs_dir(root: &Path) -> PathBuf {
    updates_dir(root).join("jobs")
}
pub fn default_directory(root: &Path, key: &str) -> Option<PathBuf> {
    let data = data_dir(root);
    match key {
        "mods" => Some(root.join("mods")),
        "state" => Some(data),
        "cache" => Some(data.join("cache")),
        "exports" => Some(data.join("exports")),
        "runtime" => Some(data.join("runtime")),
        "logs" => Some(data.join("logs")),
        "updates" => Some(data.join("updates")),
        "ui" => Some(data.join("ui")),
        _ => None,
    }
}
pub fn mods_dir(root: &Path) -> PathBuf {
    // The default matches EML's game-root directory; users can select another
    // location in the Modloader settings.
    configured_dir(root, "mods")
        .or_else(|| default_directory(root, "mods"))
        .unwrap()
}
pub fn cache_dir(root: &Path) -> PathBuf {
    configured_dir(root, "cache")
        .or_else(|| default_directory(root, "cache"))
        .unwrap()
}
pub fn export_dir(root: &Path) -> PathBuf {
    configured_dir(root, "exports").unwrap_or_else(|| default_directory(root, "exports").unwrap())
}
pub fn loader_executable(root: &Path) -> PathBuf {
    data_dir(root).join("shroudforge.exe")
}
pub fn updater_executable(root: &Path) -> PathBuf {
    data_dir(root).join("shroudforge-updater.exe")
}
pub fn updater_available(root: &Path) -> bool {
    let missing_since = UPDATER_MISSING_SINCE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut missing_since = missing_since
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if updater_executable(root).is_file() {
        missing_since.remove(root);
        return true;
    }
    let first_missing_at = *missing_since
        .entry(root.to_path_buf())
        .or_insert_with(Instant::now);
    first_missing_at.elapsed() < Duration::from_secs(UPDATER_REPLACEMENT_GRACE_SECONDS)
}
pub fn shroudforge_program_files_present(root: &Path) -> bool {
    [
        "dbghelp.dll",
        "dinput8.dll",
        "winmm.dll",
        "shroudforge/version.json",
        "shroudforge/shroudforge.exe",
        "shroudforge/shroudforge-updater.exe",
        "shroudforge/shroudforge-runtime.dll",
        "shroudforge/kfc-runtime.dll",
    ]
    .iter()
    .any(|relative| root.join(relative).is_file())
}
pub fn runtime_library(root: &Path) -> PathBuf {
    data_dir(root).join("shroudforge-runtime.dll")
}
pub fn native_runtime_library(root: &Path) -> PathBuf {
    data_dir(root).join("kfc-runtime.dll")
}
pub fn runtime_profiles_dir(root: &Path) -> PathBuf {
    runtime_dir(root).join("profiles")
}
pub fn mod_profiles_dir(root: &Path) -> PathBuf {
    runtime_profiles_dir(root).join("mod-profiles")
}
pub fn diagnostic_jobs_dir(root: &Path) -> PathBuf {
    runtime_dir(root).join("diagnostics").join("jobs")
}
pub fn startup_asset_lock(root: &Path) -> PathBuf {
    runtime_dir(root).join("startup-assets.lock")
}
pub fn runtime_dir(root: &Path) -> PathBuf {
    configured_dir(root, "runtime")
        .or_else(|| default_directory(root, "runtime"))
        .unwrap()
}
pub fn config_dir(root: &Path) -> PathBuf {
    data_dir(root).join("config")
}
pub fn loader_config(root: &Path) -> PathBuf {
    config_dir(root).join("modloader-config.json")
}
pub fn version_file(root: &Path) -> PathBuf {
    data_dir(root).join("version.json")
}
pub fn current_log(root: &Path) -> PathBuf {
    logs_dir(root).join(if is_dedicated_server(root) {
        "shroudforge-server.log"
    } else {
        "shroudforge-client.log"
    })
}

fn is_dedicated_server(root: &Path) -> bool {
    std::env::current_exe().ok().is_some_and(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                [
                    "enshrouded_server.exe",
                    "enshrouded_server",
                    "enshrouded_server.x86_64",
                ]
                .iter()
                .any(|server| name.eq_ignore_ascii_case(server))
            })
    }) || (has_server_binary(root) && !root.join("enshrouded.exe").is_file())
}

fn has_server_binary(root: &Path) -> bool {
    [
        "enshrouded_server.exe",
        "enshrouded_server",
        "enshrouded_server.x86_64",
    ]
    .iter()
    .any(|name| root.join(name).is_file())
}
pub fn logs_dir(root: &Path) -> PathBuf {
    configured_dir(root, "logs")
        .or_else(|| default_directory(root, "logs"))
        .unwrap()
}
pub fn updates_dir(root: &Path) -> PathBuf {
    configured_dir(root, "updates")
        .or_else(|| default_directory(root, "updates"))
        .unwrap()
}
pub fn ui_data_dir(root: &Path) -> PathBuf {
    configured_dir(root, "ui")
        .or_else(|| default_directory(root, "ui"))
        .unwrap()
}
pub fn state_dir(root: &Path) -> PathBuf {
    configured_dir(root, "state")
        .or_else(|| default_directory(root, "state"))
        .unwrap()
}
pub fn state_file(root: &Path) -> PathBuf {
    state_dir(root).join("state.json")
}
pub fn webview_profile(root: &Path) -> PathBuf {
    ui_data_dir(root).join("webview2-profile")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_packages_are_a_sibling_of_loader_data() {
        let root = Path::new("game");

        assert_eq!(mods_dir(root), root.join("mods"));
        assert_eq!(data_dir(root), root.join("shroudforge"));
    }
}
