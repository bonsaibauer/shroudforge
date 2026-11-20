use std::fs;
use std::path::Path;

/// Read the installed ShroudForge release version from its managed manifest.
pub fn installed_version(root: &Path) -> String {
    fs::read(shroudforge_loader_config::paths::version_file(root))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value["version"].as_str().map(str::trim).map(str::to_owned))
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into())
}
