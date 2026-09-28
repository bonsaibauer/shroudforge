//! Canonical paths for game data and mutable ShroudForge state.
use std::path::{Path, PathBuf};

pub fn data_dir(root: &Path) -> PathBuf {
    root.join("shroudforge")
}
pub fn mods_dir(root: &Path) -> PathBuf {
    // Match EML's game-root package directory. Loader-owned data remains under
    // `shroudforge/`; mod packages sit beside it so either loader can discover them.
    root.join("mods")
}
pub fn cache_dir(root: &Path) -> PathBuf {
    data_dir(root).join("cache")
}
pub fn export_dir(root: &Path) -> PathBuf {
    data_dir(root).join("exports")
}
pub fn loader_executable(root: &Path) -> PathBuf {
    data_dir(root).join("shroudforge.exe")
}
pub fn updater_executable(root: &Path) -> PathBuf {
    data_dir(root).join("shroudforge-updater.exe")
}
pub fn runtime_library(root: &Path) -> PathBuf {
    data_dir(root).join("shroudforge-runtime.dll")
}
pub fn native_runtime_library(root: &Path) -> PathBuf {
    data_dir(root).join("kfc-runtime.dll")
}
pub fn startup_asset_lock(root: &Path) -> PathBuf {
    data_dir(root).join("runtime/startup-assets.lock")
}
pub fn config_dir(root: &Path) -> PathBuf {
    data_dir(root).join("config")
}
pub fn version_file(root: &Path) -> PathBuf {
    data_dir(root).join("version.json")
}
pub fn current_log(root: &Path) -> PathBuf {
    data_dir(root).join("shroudforge.log")
}
pub fn logs_dir(root: &Path) -> PathBuf {
    data_dir(root).join("logs")
}
pub fn updates_dir(root: &Path) -> PathBuf {
    data_dir(root).join("updates")
}
pub fn ui_data_dir(root: &Path) -> PathBuf {
    data_dir(root).join("ui")
}
pub fn webview_profile(root: &Path) -> PathBuf {
    config_dir(root).join("webview2-profile")
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
