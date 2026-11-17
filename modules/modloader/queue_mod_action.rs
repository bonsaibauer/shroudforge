use crate::package_lookup::find_mod_package;
use crate::package_lookup::valid_identifier;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

/// Queue a declared action for the runtime attached to this installation.
pub fn queue_mod_action(root: &Path, id: &str, action: &str) -> Result<(), String> {
    if !valid_identifier(id) || !valid_identifier(action) {
        return Err("invalid mod action".into());
    }
    let package = find_mod_package(root, id).ok_or("mod package not found")?;
    let manifest = crate::config::read_manifest_path(root, &package)?;
    let declared = manifest
        .groups
        .iter()
        .any(|group| group.actions.iter().any(|item| item.id == action));
    if !declared {
        return Err("mod action is not declared by the package".into());
    }
    let actions = crate::paths::ui_data_dir(root).join("actions");
    fs::create_dir_all(&actions).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    crate::config::write_json(
        &actions.join(format!(
            "{}-{}-{}.json",
            id,
            timestamp.as_nanos(),
            std::process::id()
        )),
        &serde_json::json!({
            "modId": id,
            "action": action,
            "createdAt": timestamp.as_secs()
        }),
    )
}
