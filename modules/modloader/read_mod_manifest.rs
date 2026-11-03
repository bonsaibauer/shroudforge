#[cfg(test)]
use crate::parse_manifest;
use crate::{FileSystem, ModManifest, validate_mod_locale_catalogs};
use crate::{infer_api_contract, parse_manifest_with_extension};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::{io::Read, path::Path};
fn read_package_json(fs: &mut FileSystem, name: &str) -> Result<Value, String> {
    let reader = fs.read_file(name).map_err(|e| format!("{name}: {e}"))?;
    // Buffered: serde_json reads a bare file one byte per system call.
    serde_json::from_reader(std::io::BufReader::new(reader)).map_err(|e| format!("{name}: {e}"))
}

pub fn read_manifest(root: &Path, fs: &mut FileSystem) -> Result<ModManifest, String> {
    let mut value = read_package_json(fs, "mod.json")?;
    let mut extended = match fs.read_file("extended.mod.json") {
        Ok(reader) => Some(
            serde_json::from_reader(std::io::BufReader::new(reader))
                .map_err(|error| format!("extended.mod.json: {error}"))?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("extended.mod.json: {error}")),
    };
    apply_manifest_profile(root, fs, &mut value, &mut extended);
    let mut manifest = parse_manifest_with_extension(root, value, extended)?;
    let mut source = String::new();
    collect_lua_sources(fs, camino::Utf8Path::new("src"), &mut source)?;
    infer_api_contract(&mut manifest, &source);
    manifest.locales = crate::read_mod_locale_catalogs::read_mod_locale_catalogs(root, fs)?;
    validate_mod_locale_catalogs(&manifest)?;
    Ok(manifest)
}

fn apply_manifest_profile(
    root: &Path,
    fs: &mut FileSystem,
    mod_json: &mut Value,
    extended_mod_json: &mut Option<Value>,
) {
    let Some(package_directory) = fs.root().file_name() else {
        return;
    };
    let Some(mod_id) = mod_json.get("id").and_then(Value::as_str) else {
        return;
    };
    let profile = crate::mod_profiles::profile_for_package(root, package_directory, Some(mod_id));
    let Some(profile) = profile else {
        return;
    };
    if let Some(mut overlay) = profile.mod_json {
        if let Some(additions) = overlay
            .as_object_mut()
            .and_then(|object| object.remove("capabilities"))
        {
            let existing = mod_json.get("capabilities");
            // Preserve invalid source values for the normal manifest validator.
            if existing.is_none_or(Value::is_array) {
                let mut capabilities = Vec::new();
                for capability in existing
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .chain(additions.as_array().into_iter().flatten())
                {
                    if !capabilities.contains(capability) {
                        capabilities.push(capability.clone());
                    }
                }
                mod_json["capabilities"] = Value::Array(capabilities);
            }
        }
        merge_profile_object(mod_json, &overlay);
    }
    if let Some(mut overlay) = profile.extended_mod_json {
        // Mod activation is user-controlled, so adapters must never override it.
        if let Some(fields) = overlay.as_object_mut() {
            fields.remove("enabled");
        }
        let extended = extended_mod_json.get_or_insert_with(|| {
            serde_json::json!({"schemaVersion": 1, "enabled": false})
        });
        merge_profile_object(extended, &overlay);
    }
    fs.apply_profile_files(&profile.package_files);
    tracing::debug!(target: "shroudforge::dll_manager", mod_id = %profile.mod_id,
        profile_id = %profile.id, "Applied compatibility profile to in-memory manifests");
}

fn merge_profile_object(target: &mut Value, overlay: &Value) {
    let (Some(target), Some(overlay)) = (target.as_object_mut(), overlay.as_object()) else {
        return;
    };
    for (key, value) in overlay {
        match (target.get_mut(key), value) {
            (Some(existing), Value::Object(_)) => merge_profile_object(existing, value),
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn collect_lua_sources(
    fs: &mut FileSystem,
    directory: &camino::Utf8Path,
    output: &mut String,
) -> Result<(), String> {
    let entries = match fs.read_directory(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    for entry in entries.into_iter().flatten() {
        if fs.is_directory(&entry) {
            collect_lua_sources(fs, &entry, output)?;
        } else if entry.extension() == Some("lua") {
            let Ok(mut file) = fs.read_file(&entry) else {
                continue;
            };
            file.read_to_string(output)
                .map_err(|error| error.to_string())?;
            output.push('\n');
        }
    }
    Ok(())
}

pub fn read_manifest_path(root: &Path, package: &Path) -> Result<ModManifest, String> {
    let path = camino::Utf8Path::from_path(package).ok_or("package path is not UTF-8")?;
    let mut fs = if package.is_dir() {
        FileSystem::new_disk(path)
    } else {
        FileSystem::new_zip(path)
    }
    .map_err(|e| e.to_string())?;
    read_manifest(root, &mut fs)
}

pub fn valid_id(id: &str) -> bool {
    crate::package_lookup::valid_identifier(id) && id != "." && id != ".."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eml_manifest_remains_valid_without_shroudforge_extension() {
        let root = Path::new(".");
        let manifest = parse_manifest(
            root,
            json!({
                "id": "eml-example",
                "name": "EML Example",
                "version": "1.2.3",
                "capabilities": ["patch"]
            }),
        )
        .expect("an EML manifest remains loadable on its own");

        assert_eq!(manifest.id, "eml-example");
        assert!(!manifest.enabled);
        assert!(manifest.settings.is_empty());
        assert_eq!(manifest.capabilities, vec![crate::Capability::Patch]);
    }

    #[test]
    fn extension_settings_and_groups_are_read_from_the_neighbor_manifest() {
        let manifest = parse_manifest_with_extension(
            Path::new("."),
            json!({
                "id": "sf-example",
                "name": "SF Example",
                "version": "1.0.0",
                "capabilities": ["runtime"]
            }),
            Some(json!({
                "schemaVersion": 1,
                "enabled": true,
                "settings": {
                    "enabledFeature": {
                        "value": true,
                        "label": "Enable feature",
                        "description": "Run the feature.",
                        "control": "toggle"
                    }
                },
                "groups": [{
                    "label": "Features",
                    "settings": ["enabledFeature"],
                    "actions": [{"id": "reset", "label": "Reset", "confirm": "Reset this mod?"}]
                }]
            })),
        )
        .expect("the ShroudForge extension is parsed with the EML manifest");

        assert!(manifest.enabled);
        assert_eq!(manifest.setting_values["enabledFeature"], true);
        assert_eq!(manifest.settings[0].label, "Enable feature");
        assert_eq!(manifest.groups[0].settings, vec!["enabledFeature"]);
        assert_eq!(manifest.groups[0].actions[0].id, "reset");
    }

    #[test]
    fn explicit_setting_schedule_survives_hybrid_api_inference() {
        let mut manifest = parse_manifest_with_extension(
            Path::new("."),
            json!({
                "id": "hybrid-example",
                "name": "Hybrid Example",
                "version": "1.0.0",
                "capabilities": ["patch", "runtime"]
            }),
            Some(json!({
                "schemaVersion": 1,
                "enabled": true,
                "settings": {
                    "freeRecipes": {
                        "value": true,
                        "applyAt": "restart",
                        "restartRequired": true
                    },
                    "liveScale": {
                        "value": 100,
                        "applyAt": "live",
                        "restartRequired": false
                    },
                    "inferredRuntimeValue": {"value": true}
                }
            })),
        )
        .expect("per-setting runtime schedules are parsed");

        infer_api_contract(
            &mut manifest,
            "game.assets.get_resources_by_type('type'); return {on_update=update}",
        );

        let schedules: Vec<_> = manifest
            .settings
            .iter()
            .map(|setting| {
                (
                    setting.key.as_str(),
                    setting.apply_at.as_str(),
                    setting.restart_required,
                )
            })
            .collect();
        assert!(schedules.contains(&("freeRecipes", "restart", true)));
        assert!(schedules.contains(&("liveScale", "live", false)));
        assert!(schedules.contains(&("inferredRuntimeValue", "live", false)));
    }

    #[test]
    fn extension_targets_apply_to_all_packages_without_changing_mod_json() {
        let manifest = parse_manifest_with_extension(
            Path::new("."),
            json!({
                "id": "shared-runtime-mod",
                "name": "Shared Runtime Mod",
                "version": "1.0.0",
                "capabilities": ["runtime"]
            }),
            Some(json!({"schemaVersion":1,"enabled":true,"targets":["client","server"]})),
        )
        .expect("targets are read from extended.mod.json");

        assert!(matches!(manifest.target, crate::ModTarget::ClientServer));
    }

    #[test]
    fn legacy_eml_without_extension_defaults_to_client_and_server() {
        let mut manifest = parse_manifest(
            Path::new("."),
            json!({
                "id": "legacy-runtime-mod",
                "name": "Legacy Runtime Mod",
                "version": "1.0.0",
                "capabilities": ["runtime"]
            }),
        )
        .expect("legacy EML manifest remains valid");

        infer_api_contract(
            &mut manifest,
            "runtime.patch.set_enabled('runtime.patch.example', true)",
        );

        assert!(matches!(manifest.target, crate::ModTarget::ClientServer));
    }

    #[test]
    fn mod_targets_match_only_the_declared_processes() {
        assert!(crate::ModTarget::Client.supports_process(false));
        assert!(!crate::ModTarget::Client.supports_process(true));
        assert!(!crate::ModTarget::Server.supports_process(false));
        assert!(crate::ModTarget::Server.supports_process(true));
        assert!(crate::ModTarget::ClientServer.supports_process(false));
        assert!(crate::ModTarget::ClientServer.supports_process(true));
    }

    #[test]
    fn shipped_mod_packages_have_valid_eml_and_extension_manifests() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("modloader crate lives under modules");
        let mods = repository_root.join("mods");
        let packages = std::fs::read_dir(&mods)
            .expect("shipped mods directory exists")
            .map(|entry| entry.expect("mod directory entry is readable").path())
            .filter(|path| path.join("mod.json").is_file())
            .collect::<Vec<_>>();
        assert!(!packages.is_empty(), "at least one shipped mod is checked");

        for package in packages {
            read_manifest_path(repository_root, &package)
                .unwrap_or_else(|error| panic!("{}: {error}", package.display()));
        }
    }
}
