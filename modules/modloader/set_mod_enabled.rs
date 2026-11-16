use std::path::Path;

pub fn set_mod_enabled(root: &Path, id: &str, enabled: bool, revision: &str) -> Result<(), String> {
    crate::update_mod_configuration::update_mod_configuration(
        root,
        id,
        Some(revision),
        |extension, manifest| {
            let needs_hotkey_validation = enabled
                && !manifest.enabled
                && manifest
                    .settings
                    .iter()
                    .any(|setting| matches!(setting.control, Some(crate::SettingControl::Keybind)));
            if needs_hotkey_validation {
                let config = crate::config::read_loader(root)?;
                crate::validate_hotkey_configuration(
                    root,
                    &config,
                    Some((id, &serde_json::json!({}), Some(true))),
                )?;
            }
            extension["enabled"] = enabled.into();
            Ok(())
        },
    )
}
