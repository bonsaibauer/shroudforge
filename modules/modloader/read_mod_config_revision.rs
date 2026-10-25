use crate::package_lookup::find_mod_package;
use std::path::Path;

/// Revision of a mod's saved state (enabled and setting values), the one the
/// Modloader shows and `update_mod_configuration` checks before a save, so
/// the page can save again right away without waiting for its next refresh.
pub fn read_mod_config_revision(root: &Path, id: &str) -> Result<String, String> {
    let package = find_mod_package(root, id).ok_or("mod package not found")?;
    let manifest = crate::config::read_manifest_path(root, &package)?;
    Ok(crate::config_revision(
        &serde_json::json!({"enabled":manifest.enabled,"settings":manifest.setting_values}),
    ))
}
