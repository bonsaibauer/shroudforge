use std::path::Path;

pub fn set_mod_enabled(root: &Path, id: &str, enabled: bool, revision: &str) -> Result<(), String> {
    let target = crate::package_lookup::find_mod_package_by_reference(root, id)
        .ok_or("mod package not found")?;
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
                    Some((&manifest.id, &serde_json::json!({}), Some(true))),
                )?;
            }
            extension["enabled"] = enabled.into();
            Ok(())
        },
    )?;
    if enabled {
        let manifest = crate::config::read_manifest_path(root, &target)?;
        let locations = crate::modpack::package_locations(&crate::paths::mods_dir(root))?;
        for location in locations {
            if location.path == target { continue }
            let Ok(other) = crate::config::read_manifest_path(root, &location.path) else { continue };
            if other.id == manifest.id && other.enabled {
                if let Err(error) = crate::update_mod_configuration::update_mod_configuration(
                    root, &location.key(), None, |extension, _| {
                        extension["enabled"] = false.into();
                        Ok(())
                    },
                ) {
                    let _ = crate::update_mod_configuration::update_mod_configuration(
                        root, id, None, |extension, _| {
                            extension["enabled"] = false.into();
                            Ok(())
                        },
                    );
                    return Err(format!("could not disable another copy of mod '{}': {error}", manifest.id));
                }
            }
        }
    }
    Ok(())
}
