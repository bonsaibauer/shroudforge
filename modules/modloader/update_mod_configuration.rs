use crate::config_revision;
use crate::package_files::{package_has_file, read_package_file, write_package_json};
use crate::package_lookup::find_mod_package;
use std::path::Path;

pub(crate) fn update_mod_configuration(
    root: &Path,
    id: &str,
    expected_revision: &str,
    update: impl FnOnce(&mut serde_json::Value) -> Result<(), String>,
) -> Result<(), String> {
    let package = find_mod_package(root, id).ok_or("mod package not found")?;
    let manifest = crate::config::read_manifest_path(root, &package)?;
    let current_state =
        serde_json::json!({"enabled":manifest.enabled,"settings":manifest.setting_values});
    if config_revision(&current_state) != expected_revision {
        return Err("Mod settings changed since they were displayed. Reload before saving".into());
    }
    let mut extension = match read_package_file(&package, "extended.mod.json") {
        Ok(bytes) => {
            let mut extension = serde_json::from_slice::<serde_json::Value>(&bytes)
                .map_err(|error| error.to_string())?;
            prepare_existing_extension(&mut extension)?;
            extension
        }
        Err(_error) if !package_has_file(&package, "extended.mod.json")? => {
            crate::ensure_eml_extension::ensure_eml_extension(&package, &manifest)?
        }
        Err(error) => return Err(error),
    };
    update(&mut extension)?;
    crate::parse_mod_manifest::validate_extended_mod_document(root, &extension)?;
    let wire_manifest = serde_json::from_slice(&read_package_file(&package, "mod.json")?)
        .map_err(|error| error.to_string())?;
    crate::config::parse_manifest_with_extension(root, wire_manifest, Some(extension.clone()))?;
    write_package_json(&package, "extended.mod.json", &extension)
}

fn prepare_existing_extension(extension: &mut serde_json::Value) -> Result<(), String> {
    let launcher_is_eml = extension
        .get("launcher")
        .and_then(serde_json::Value::as_str)
        == Some("EML");
    let object = extension
        .as_object_mut()
        .ok_or("extended.mod.json must be an object")?;
    object
        .entry("settings")
        .or_insert_with(|| serde_json::json!({}));
    if !object.contains_key("targets") {
        let targets = if launcher_is_eml {
            serde_json::json!(["client", "server"])
        } else {
            serde_json::json!(["client"])
        };
        let old = std::mem::take(object);
        let mut prepared = serde_json::Map::new();
        for (key, value) in old {
            let insert_targets = key == "enabled";
            prepared.insert(key, value);
            if insert_targets {
                prepared.insert("targets".into(), targets.clone());
            }
        }
        *object = prepared;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_package(extension: Option<&[u8]>) -> (std::path::PathBuf, std::path::PathBuf) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("shroudforge-mod-configuration-{suffix}"));
        let package = shroudforge_loader_config::paths::mods_dir(&root).join("example");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("mod.json"),
            br#"{"id":"example","name":"Example","version":"1.0.0"}"#,
        )
        .unwrap();
        if let Some(extension) = extension {
            std::fs::write(package.join("extended.mod.json"), extension).unwrap();
        }
        (root, package)
    }

    #[test]
    fn updating_shroudforge_configuration_keeps_its_provenance() {
        let (root, package) = test_package(Some(
            br#"{"schemaVersion":1,"enabled":false,"launcher":"SF"}"#,
        ));
        let revision = config_revision(&serde_json::json!({"enabled":false,"settings":{}}));

        update_mod_configuration(&root, "example", &revision, |extension| {
            extension["enabled"] = true.into();
            Ok(())
        })
        .unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(package.join("extended.mod.json")).unwrap())
                .unwrap();
        assert_eq!(saved["enabled"], true);
        assert_eq!(saved["launcher"], "SF");
        assert_eq!(saved["targets"], serde_json::json!(["client"]));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn first_change_to_legacy_eml_mod_creates_its_compatibility_extension() {
        let (root, package) = test_package(None);
        let revision = config_revision(&serde_json::json!({"enabled":false,"settings":{}}));

        update_mod_configuration(&root, "example", &revision, |extension| {
            extension["enabled"] = true.into();
            Ok(())
        })
        .unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(package.join("extended.mod.json")).unwrap())
                .unwrap();
        assert_eq!(saved["enabled"], true);
        assert_eq!(saved["launcher"], "EML");
        assert_eq!(saved["targets"], serde_json::json!(["client", "server"]));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn saving_preset_settings_keeps_the_presets() {
        let (root, package) = test_package(Some(
            br#"{"schemaVersion":1,"enabled":true,"settings":{
                "quality":{"value":"low","options":{"low":"Low","high":"High","custom":"Custom"}},
                "range":{"value":64}
            },"presets":{"quality":{"settings":["range"],"values":{"low":{"range":64},"high":{"range":256}}}}}"#,
        ));
        let revision = config_revision(&serde_json::json!({
            "enabled": true,
            "settings": {"quality": "low", "range": 64}
        }));

        crate::save_mod_settings(
            &root,
            "example",
            &serde_json::json!({"quality": "high", "range": 256}),
            &revision,
        )
        .unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(package.join("extended.mod.json")).unwrap())
                .unwrap();
        assert_eq!(saved["settings"]["quality"]["value"], "high");
        assert_eq!(saved["settings"]["range"]["value"], 256);
        assert_eq!(saved["presets"]["quality"]["values"]["high"]["range"], 256);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_configuration_revision_does_not_write() {
        let (root, package) = test_package(Some(
            br#"{"schemaVersion":1,"enabled":false,"launcher":"SF"}"#,
        ));

        let result = update_mod_configuration(&root, "example", "stale", |extension| {
            extension["enabled"] = true.into();
            Ok(())
        });

        assert!(result.is_err());
        let saved = std::fs::read(package.join("extended.mod.json")).unwrap();
        assert_eq!(
            saved,
            br#"{"schemaVersion":1,"enabled":false,"launcher":"SF"}"#
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
