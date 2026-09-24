//! Canonical paths for mutable ShroudForge data.
use std::path::{Path, PathBuf};

pub fn data_dir(root: &Path) -> PathBuf {
    root.join("shroudforge")
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
