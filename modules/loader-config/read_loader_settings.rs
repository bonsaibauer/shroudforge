use crate::config::*;
use serde_json::Value;
use std::{fs, path::Path};

pub fn read_loader_defaults() -> Result<Value, String> {
    serde_json::from_str(include_str!("loader.default.json")).map_err(|error| error.to_string())
}

pub fn read_loader(root: &Path) -> Result<Value, String> {
    let result = read_loader_unvalidated(root)?;
    validate_document(root, crate::documents::LOADER, &result)?;
    Ok(result)
}

pub(crate) fn read_loader_unvalidated(root: &Path) -> Result<Value, String> {
    let path = document_path(root, crate::documents::LOADER);
    let mut result = read_loader_defaults()?;
    let mut loaded = match fs::read(&path) {
        Ok(bytes) => {
            Some(serde_json::from_slice::<Value>(&bytes).map_err(|error| error.to_string())?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    if let Some(loaded) = loaded.as_mut() {
        if let Some(modules) = loaded
            .pointer_mut("/modules")
            .and_then(Value::as_object_mut)
        {
            for name in ["worldEditor", "modloaderUi"] {
                let settings = modules
                    .entry(name)
                    .or_insert_with(|| Value::Object(Default::default()));
                if let Some(settings) = settings.as_object_mut() {
                    settings.entry("enabled").or_insert(Value::Bool(true));
                }
            }
            let network = modules
                .entry("network")
                .or_insert_with(|| Value::Object(Default::default()));
            if let Some(network) = network.as_object_mut() {
                network.entry("enabled").or_insert(Value::Bool(true));
            }
            let updates = modules
                .entry("updates")
                .or_insert_with(|| Value::Object(Default::default()));
            if let Some(updates) = updates.as_object_mut() {
                updates.entry("enabled").or_insert(Value::Bool(true));
            }
        }
        let legacy_system = loaded
            .pointer("/modules/updates/system")
            .cloned()
            .unwrap_or_default();
        let legacy_mods = loaded
            .pointer("/modules/updates/mods")
            .cloned()
            .unwrap_or_default();
        if let Some(updates) = loaded
            .pointer_mut("/modules/updates")
            .and_then(Value::as_object_mut)
        {
            let automatic = updates
                .entry("automatic")
                .or_insert_with(|| Value::Object(Default::default()));
            if let Some(automatic) = automatic.as_object_mut() {
                automatic.entry("enabled").or_insert_with(|| {
                    Value::Bool(
                        legacy_system
                            .get("enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(true)
                            && legacy_mods
                                .get("enabled")
                                .and_then(Value::as_bool)
                                .unwrap_or(true),
                    )
                });
                automatic.entry("checkMinutes").or_insert_with(|| {
                    let minutes = legacy_system
                        .get("checkMinutes")
                        .or_else(|| legacy_mods.get("checkMinutes"))
                        .and_then(Value::as_u64)
                        .unwrap_or(60)
                        .clamp(5, 1440);
                    Value::from(minutes)
                });
                automatic.entry("queueUpdates").or_insert(Value::Bool(true));
            }
            for name in ["system", "mods"] {
                if let Some(settings) = updates.get_mut(name).and_then(Value::as_object_mut) {
                    settings.remove("enabled");
                    settings.remove("checkMinutes");
                }
            }
        }
        if let Some(diagnostics) = loaded
            .pointer_mut("/modules/runtimeDiagnostics")
            .and_then(Value::as_object_mut)
        {
            if !diagnostics.contains_key("sessionActive") {
                if let Some(active) = diagnostics.get("enabled").cloned() {
                    diagnostics.insert("sessionActive".into(), active);
                }
            }
        }
        if let Some(provider) = loaded
            .pointer_mut("/catalog/provider")
            .and_then(Value::as_object_mut)
        {
            for key in [
                "enabled",
                "checkMinutes",
                "includePrereleases",
                "lastCheckAt",
                "latestRelease",
            ] {
                provider.remove(key);
            }
        }
        if let Some(system) = loaded
            .pointer_mut("/modules/updates/system")
            .and_then(Value::as_object_mut)
        {
            system.remove("channel");
        }
    }
    fn merge(target: &mut Value, source: Value) {
        match (target, source) {
            (Value::Object(target), Value::Object(source)) => {
                for (key, value) in source {
                    merge(target.entry(key).or_insert(Value::Null), value);
                }
            }
            (target, source) => *target = source,
        }
    }
    if let Some(loaded) = loaded {
        merge(&mut result, loaded);
    }
    if let Some(settings) = result
        .pointer_mut("/modules/runtimeDiagnostics")
        .and_then(Value::as_object_mut)
    {
        settings.remove("areas");
        if settings.get("jobId").and_then(Value::as_str) == Some("legacy.custom-areas") {
            settings.insert("jobId".into(), "runtime.overview".into());
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn modules_are_enabled_by_default_and_windows_start_closed() {
        let root = tempfile::tempdir().unwrap();
        let updater = crate::paths::updater_executable(root.path());
        fs::create_dir_all(updater.parent().unwrap()).unwrap();
        fs::write(updater, b"").unwrap();
        crate::config::initialize_loader_config(root.path()).unwrap();
        let config = read_loader(root.path()).unwrap();

        for module in [
            "debugConsole",
            "worldEditor",
            "modloaderUi",
            "network",
            "runtimeDiagnostics",
            "updates",
        ] {
            assert_eq!(config["modules"][module]["enabled"], true, "{module}");
        }
        assert_eq!(
            crate::config::window_state(root.path()),
            serde_json::json!({})
        );
    }

    #[test]
    fn explicitly_disabled_module_stays_disabled() {
        let root = tempfile::tempdir().unwrap();
        let config_path = crate::paths::loader_config(root.path());
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            config_path,
            br#"{"schemaVersion":1,"modules":{"network":{"enabled":false}}}"#,
        )
        .unwrap();

        let config = read_loader(root.path()).unwrap();

        assert_eq!(config["modules"]["network"]["enabled"], false);
        for module in [
            "debugConsole",
            "worldEditor",
            "modloaderUi",
            "runtimeDiagnostics",
            "updates",
        ] {
            assert_eq!(config["modules"][module]["enabled"], true, "{module}");
        }
    }
}
