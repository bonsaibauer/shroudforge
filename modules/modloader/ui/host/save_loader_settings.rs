use super::loader_settings_input::UiSettings;
use super::validate_message_name::normalize_message_name;
use std::path::Path;

pub(crate) fn save_settings(root: &Path, settings: &UiSettings) -> Result<(), String> {
    if settings
        .module_preferences
        .as_ref()
        .and_then(|preferences| preferences.pointer("/updates/enabled"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        && !shroudforge_loader_config::paths::updater_available(root)
    {
        return Err(super::report_command_result::missing_updater_message(root));
    }
    let base_url = settings.base_url.trim_end_matches('/');
    let project_id = settings.project_id.trim();
    if base_url.is_empty() || project_id.is_empty() {
        return Err("update provider fields must not be empty".into());
    }
    let provider = serde_json::json!({
        "kind": "shroudedit",
        "baseUrl": base_url,
        "projectId": project_id,
        "loader": "shroudforge"
    });
    shroudforge_loader_config::config::update_loader(root, |value| {
        let previous_modules = value["modules"].clone();
        if let Some(preferences) = &settings.module_preferences {
            for (module, keys) in [
                (
                    "debugConsole",
                    &[
                        "enabled",
                        "popupMode",
                        "popupCooldownSeconds",
                        "defaultSource",
                        "levelFilter",
                        "autoScroll",
                        "toggleKey",
                        "refreshMilliseconds",
                        "tailBytes",
                    ][..],
                ),
                (
                    "modloaderUi",
                    &["enabled", "startPage", "toggleKey", "refreshMilliseconds"][..],
                ),
                (
                    "wiki",
                    &[
                        "enabled",
                        "pageSize",
                        "maxResultsPerQuery",
                        "refreshIntervalMinutes",
                    ][..],
                ),
                (
                    "worldEditor",
                    &["enabled", "toggleKey", "refreshMilliseconds"][..],
                ),
                ("gameProfileDiscovery", &["enabled"][..]),
                (
                    "runtimeDiagnostics",
                    &[
                        "enabled",
                        "jobId",
                        "intervalMilliseconds",
                        "maximumDurationSeconds",
                        "slowCallbackMilliseconds",
                        "continuous",
                        "onlyChanges",
                    ][..],
                ),
            ] {
                for key in keys {
                    if let Some(setting) = preferences.get(module).and_then(|value| value.get(*key))
                    {
                        if module == "runtimeDiagnostics"
                            && *key == "enabled"
                            && setting == &serde_json::Value::Bool(false)
                        {
                            value["modules"][module]["sessionActive"] = false.into();
                        }
                        if module == "runtimeDiagnostics" && *key == "jobId" {
                            let job_id =
                                setting.as_str().ok_or("diagnostic job id must be text")?;
                            let catalog = shroudforge_runtime_diagnostics::diagnostic_job_catalog(
                                root,
                                &value["modules"]["runtimeDiagnostics"],
                            );
                            if !catalog.jobs.iter().any(|job| job.id == job_id) {
                                return Err(format!("diagnostic job was not found: {job_id}"));
                            }
                            value["modules"][module]["jobId"] = setting.clone();
                            continue;
                        }
                        value["modules"][module][*key] = setting.clone();
                    }
                }
                if let Some(position) = preferences
                    .get(module)
                    .and_then(|value| value.pointer("/window/position"))
                {
                    value["modules"][module]["window"]["position"] = position.clone();
                }
            }
            if let Some(enabled) = preferences.pointer("/updates/enabled") {
                value["modules"]["updates"]["enabled"] = enabled.clone();
            }
            for key in ["enabled", "checkMinutes", "queueUpdates"] {
                if let Some(setting) = preferences.pointer(&format!("/updates/automatic/{key}")) {
                    value["modules"]["updates"]["automatic"][key] = setting.clone();
                }
            }
            for key in ["includePrereleases"] {
                if let Some(setting) = preferences.pointer(&format!("/updates/system/{key}")) {
                    value["modules"]["updates"]["system"][key] = setting.clone();
                }
            }
            for key in ["automaticCleanup", "retentionDays"] {
                if let Some(setting) = preferences.pointer(&format!("/updates/recycleBin/{key}")) {
                    value["modules"]["updates"]["recycleBin"][key] = setting.clone();
                }
            }
        }
        let logging = value
            .get_mut("logging")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("logging config must be an object")?;
        logging.insert("minimumLevel".into(), settings.log_level.clone().into());
        let object = value
            .as_object_mut()
            .ok_or("shroudforge/config/modloader-config.json must contain an object")?;
        let exports = object
            .entry("exports")
            .or_insert_with(|| serde_json::json!({}));
        exports["enabled"] = settings.export_enabled.into();
        object.insert(
            "catalog".into(),
            serde_json::json!({ "provider": provider }),
        );
        let general = object
            .entry("general")
            .or_insert_with(|| serde_json::json!({}));
        let general = general
            .as_object_mut()
            .ok_or("general config must be an object")?;
        general.insert("compactMode".into(), settings.compact_mode.into());
        general.insert("reducedMotion".into(), settings.reduced_motion.into());
        general.insert(
            "showNativeModMenu".into(),
            settings.show_native_mod_menu.into(),
        );
        general.insert(
            "usernameMode".into(),
            if settings.username_mode == "manual" {
                "manual"
            } else {
                "auto"
            }
            .into(),
        );
        general.insert(
            "messageNameFallback".into(),
            normalize_message_name(&settings.message_name_fallback)?.into(),
        );
        let hotkeys_changed = value["modules"].as_object().is_some_and(|modules| {
            modules.iter().any(|(id, module)| {
                module.get("toggleKey").is_some_and(|binding| {
                    previous_modules
                        .get(id)
                        .and_then(|previous| previous.get("toggleKey"))
                        != Some(binding)
                        || (module.get("enabled") == Some(&serde_json::Value::Bool(true))
                            && previous_modules
                                .get(id)
                                .and_then(|previous| previous.get("enabled"))
                                == Some(&serde_json::Value::Bool(false)))
                })
            })
        });
        if hotkeys_changed {
            shroudforge_package::validate_hotkey_configuration(root, value, None)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings(preferences: serde_json::Value) -> UiSettings {
        serde_json::from_value(json!({
            "modulePreferences": preferences,
            "showNativeModMenu": true,
            "logLevel": "INFO",
            "baseUrl": "https://api.shroudedit.com",
            "projectId": "shroudforge"
        }))
        .unwrap()
    }

    #[test]
    fn game_profile_discovery_bulk_save_persists_toggle_and_preserves_missing_value() {
        let root = tempfile::tempdir().unwrap();
        save_settings(root.path(), &settings(json!({}))).unwrap();
        let saved = shroudforge_loader_config::config::read_loader(root.path()).unwrap();
        assert_eq!(saved["modules"]["gameProfileDiscovery"]["enabled"], true);
        for enabled in [true, false, true] {
            save_settings(
                root.path(),
                &settings(json!({"gameProfileDiscovery": {"enabled": enabled}})),
            )
            .unwrap();
            save_settings(root.path(), &settings(json!({}))).unwrap();
            save_settings(
                root.path(),
                &settings(json!({"gameProfileDiscovery": {}})),
            )
            .unwrap();
            let saved = shroudforge_loader_config::config::read_loader(root.path()).unwrap();
            assert_eq!(saved["modules"]["gameProfileDiscovery"]["enabled"], enabled);
        }
    }

    #[test]
    fn game_profile_discovery_bulk_save_rejects_invalid_values_without_writing() {
        let root = tempfile::tempdir().unwrap();
        save_settings(
            root.path(),
            &settings(json!({"gameProfileDiscovery": {"enabled": true}})),
        )
        .unwrap();
        let path = shroudforge_loader_config::paths::loader_config(root.path());
        let before = std::fs::read(&path).unwrap();
        for invalid in [json!("false"), json!(0), serde_json::Value::Null] {
            assert!(save_settings(
                root.path(),
                &settings(json!({"gameProfileDiscovery": {"enabled": invalid}})),
            )
            .is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }
}
