use serde_json::Value;
use std::{fs, path::Path};

const MODULES: &[&str] = &[
    "modDetection",
    "dllManager",
    "debugConsole",
    "worldEditor",
    "modloaderUi",
    "wiki",
    "network",
    "runtimeDiagnostics",
    "gameProfileDiscovery",
    "updates",
];

pub fn migrate_module_target_visibility(root: &Path) -> Result<bool, String> {
    let _lock = crate::config::installation_lock(root)?;
    let path = crate::paths::loader_config(root);
    let source: Value = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    let mut settings = crate::config::read_loader_unvalidated(root)?;
    if !settings.get("modules").is_some_and(Value::is_object) {
        return Err("loader modules settings must be an object".into());
    }
    let mut changed = false;

    for module in MODULES {
        let Some(legacy) = source.pointer(&format!("/modules/{module}/visibleInServerTarget"))
        else {
            continue;
        };
        let visible = legacy
            .as_bool()
            .ok_or_else(|| format!("modules.{module}.visibleInServerTarget must be a boolean"))?;
        let target_path = format!("/general/uiTargets/visibility/server/modules/{module}");
        if source.pointer(&target_path).is_none() {
            settings["general"]["uiTargets"]["visibility"]["server"]["modules"][*module] =
                Value::Bool(visible);
        }
        if let Some(module_settings) = settings
            .pointer_mut(&format!("/modules/{module}"))
            .and_then(Value::as_object_mut)
        {
            module_settings.remove("visibleInServerTarget");
        }
        changed = true;
    }

    if !changed {
        return Ok(false);
    }
    crate::config::validate_document(root, crate::documents::LOADER, &settings)?;
    crate::config::write_json(&path, &settings)?;
    Ok(true)
}
