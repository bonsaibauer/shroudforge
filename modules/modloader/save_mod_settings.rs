use std::path::Path;

pub fn save_mod_settings(
    root: &Path,
    id: &str,
    values: &serde_json::Value,
    revision: &str,
) -> Result<(), String> {
    crate::update_mod_configuration::update_mod_configuration(
        root,
        id,
        Some(revision),
        |extension, manifest| {
            crate::config::validate_settings(&manifest.settings, values)?;
            let mut candidate = manifest.setting_values.clone();
            for (key, value) in values.as_object().ok_or("settings must be an object")? {
                candidate[key] = value.clone();
            }
            let affects_keybind = manifest.settings.iter().any(|setting| {
                matches!(setting.control, Some(crate::SettingControl::Keybind))
                    && (candidate.get(&setting.key) != manifest.setting_values.get(&setting.key)
                        || crate::setting_is_enabled(&setting.key, &manifest.settings, &candidate)
                            != crate::setting_is_enabled(
                                &setting.key,
                                &manifest.settings,
                                &manifest.setting_values,
                            ))
            });
            if affects_keybind {
                let config = crate::config::read_loader(root)?;
                crate::validate_hotkey_configuration(root, &config, Some((id, values, None)))?;
            }
            let next = values.as_object().ok_or("settings must be an object")?;
            let settings = extension
                .get_mut("settings")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or("extended.mod.json settings must be an object")?;
            for (key, value) in next {
                let current = settings
                    .get_mut(key)
                    .ok_or_else(|| format!("unknown setting '{key}'"))?;
                if current.is_object() {
                    current["value"] = value.clone();
                } else {
                    *current = value.clone();
                }
            }
            Ok(())
        },
    )
}
