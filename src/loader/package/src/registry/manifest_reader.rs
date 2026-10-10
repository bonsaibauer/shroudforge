use serde_json::{Value, json};
use std::{io::Read, path::Path};

use crate::config::{validate_document, validate_settings};
use crate::{
    FileSystem, ModManifest, SettingDefinition, SettingGroup, SettingOption, SettingValueType,
    validate_manifest,
};

fn read_package_json(fs: &mut FileSystem, name: &str) -> Result<Value, String> {
    let reader = fs.read_file(name).map_err(|e| format!("{name}: {e}"))?;
    serde_json::from_reader(reader).map_err(|e| format!("{name}: {e}"))
}

/// The same package reader is used by discovery and the UI, for directories and ZIPs.
pub fn read_manifest(root: &Path, fs: &mut FileSystem) -> Result<ModManifest, String> {
    let value = read_package_json(fs, "mod.json")?;
    let extended = match fs.read_file("extended.mod.json") {
        Ok(reader) => Some(
            serde_json::from_reader(reader)
                .map_err(|error| format!("extended.mod.json: {error}"))?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("extended.mod.json: {error}")),
    };
    let mut manifest = parse_manifest_with_extension(root, value, extended)?;
    let mut source = String::new();
    collect_lua_sources(fs, camino::Utf8Path::new("src"), &mut source)?;
    infer_api_contract(&mut manifest, &source);
    Ok(manifest)
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

/// Derive execution phase from Lua API usage. Process targets live in
/// extended.mod.json. Legacy EML packages default to both listed processes.
pub fn infer_api_contract(manifest: &mut ModManifest, source: &str) {
    let normalized: String = strip_lua_comments(source)
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let assets = normalized.contains("runtime.require(\"game.assets.write\")")
        || normalized.contains("runtime.require('game.assets.write')")
        || [
            "game.assets.update_asset(",
            "game.assets.save_assets(",
            "game.assets.reset_assets(",
            "game.assets.create_resource(",
            "game.assets.create_content(",
        ]
        .iter()
        .any(|method| normalized.contains(method));
    let runtime = normalized.contains("runtime.require(\"runtime.lifecycle\")")
        || normalized.contains("runtime.require('runtime.lifecycle')")
        || normalized.contains("runtime.ecs.")
        || normalized.contains("runtime.world.")
        || normalized.contains("runtime.patch.")
        || normalized.contains("runtime.report_effect")
        || normalized.contains("on_load=")
        || normalized.contains("on_update=")
        || normalized.contains("on_unload=");
    // Capabilities are author-declared contract entries. Source inspection may
    // derive execution metadata, but it must never silently grant permissions.
    let apply_at = if runtime && !assets {
        "live"
    } else {
        "restart"
    };
    for setting in &mut manifest.settings {
        setting.apply_at = apply_at.into();
        setting.restart_required = apply_at != "live";
    }
}

fn strip_lua_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut quote = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;
    while let Some(character) = chars.next() {
        if line_comment {
            if character == '\n' {
                line_comment = false;
                output.push(character);
            }
            continue;
        }
        if block_comment {
            if character == ']' && chars.peek() == Some(&']') {
                chars.next();
                block_comment = false;
            }
            continue;
        }
        if let Some(delimiter) = quote {
            output.push(character);
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
            output.push(character);
            continue;
        }
        if character == '-' && chars.peek() == Some(&'-') {
            chars.next();
            if chars.peek() == Some(&'[') {
                chars.next();
                if chars.peek() == Some(&'[') {
                    chars.next();
                    block_comment = true;
                    continue;
                }
                output.push_str("--[");
                continue;
            }
            line_comment = true;
            continue;
        }
        output.push(character);
    }
    output
}

/// Canonical wire manifest -> internal UI/runtime view. Never serialize this view to disk.
pub fn parse_manifest(root: &Path, value: Value) -> Result<ModManifest, String> {
    parse_manifest_with_extension(root, value, None)
}

/// Reads an EML manifest and its optional neighboring ShroudForge extension.
pub fn parse_manifest_with_extension(
    root: &Path,
    mut value: Value,
    external: Option<Value>,
) -> Result<ModManifest, String> {
    let had_extension = external.is_some();
    let mut extension = external;
    if let Some(extension) = extension.as_mut() {
        validate_document(root, "extended-mod", extension)?;
        if let Some(object) = extension.as_object_mut() {
            object.remove("$schema");
        }
    }
    validate_document(root, "mod", &value)?;
    let extension = extension.unwrap_or_else(|| json!({}));
    let enabled = extension
        .get("enabled")
        .and_then(Value::as_bool)
        // Mods remain disabled until activation is explicitly persisted. This
        // also keeps EML packages without an extension safe by default.
        .unwrap_or(false);
    let target = match extension.get("targets").and_then(Value::as_array) {
        Some(targets) => {
            let client = targets.iter().any(|value| value == "client");
            let server = targets.iter().any(|value| value == "server");
            match (client, server) {
                (true, true) => crate::ModTarget::ClientServer,
                (false, true) => crate::ModTarget::Server,
                _ => crate::ModTarget::Client,
            }
        }
        None if extension.get("launcher").and_then(Value::as_str) == Some("EML") => {
            crate::ModTarget::ClientServer
        }
        None if had_extension => {
            crate::ModTarget::Client
        }
        None => crate::ModTarget::ClientServer,
    };
    let settings = extension
        .get("settings")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let settings_object = settings
        .as_object()
        .ok_or("extended.mod.json settings must be an object")?;
    let mut effective = serde_json::Map::new();
    let mut definitions = Vec::new();
    let empty_metadata = Value::Null;
    for (key, entry) in settings_object {
        let (value, metadata) = if entry.is_object() {
            (
                entry
                    .get("value")
                    .cloned()
                    .ok_or_else(|| format!("setting '{key}' requires value"))?,
                entry,
            )
        } else {
            (entry.clone(), &empty_metadata)
        };
        let definition = parse_setting_definition(key, &value, metadata)?;
        effective.insert(key.clone(), value);
        definitions.push(serde_json::to_value(definition).map_err(|error| error.to_string())?);
    }
    validate_settings(
        &serde_json::from_value::<Vec<SettingDefinition>>(json!(definitions.clone()))
            .map_err(|error| error.to_string())?,
        &Value::Object(effective.clone()),
    )?;
    let groups = extension
        .get("groups")
        .map(|value| {
            serde_json::from_value::<Vec<SettingGroup>>(value.clone())
                .map_err(|error| format!("extended.mod.json groups: {error}"))
        })
        .transpose()?
        .unwrap_or_default();
    let object = value.as_object_mut().ok_or("mod.json must be an object")?;
    object.remove("$schema");
    object.insert("enabled".into(), json!(enabled));
    object.insert(
        "target".into(),
        serde_json::to_value(target).map_err(|error| error.to_string())?,
    );
    object.insert("settingValues".into(), Value::Object(effective));
    object.insert("settings".into(), json!(definitions));
    object.insert("groups".into(), json!(groups));
    object.insert(
        "changelog".into(),
        extension
            .get("changelog")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    object.insert(
        "links".into(),
        extension.get("links").cloned().unwrap_or_else(|| json!({})),
    );
    let manifest: ModManifest = serde_json::from_value(value).map_err(|error| error.to_string())?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn parse_setting_definition(
    key: &str,
    value: &Value,
    metadata: &Value,
) -> Result<SettingDefinition, String> {
    let value_type = match value {
        Value::Bool(_) => SettingValueType::Boolean,
        Value::String(_) => SettingValueType::String,
        Value::Number(number) if number.as_i64().is_some() => SettingValueType::Integer,
        Value::Number(_) => SettingValueType::Number,
        Value::Array(_) => SettingValueType::Array,
        _ => {
            return Err(format!(
                "setting '{key}' value must be boolean, string, number, or array"
            ));
        }
    };
    let options = metadata
        .get("options")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .map(|(option, label)| {
                    let option_value = match value_type {
                        SettingValueType::Boolean => Value::Bool(option == "true"),
                        SettingValueType::Integer => Value::Number(
                            option
                                .parse::<i64>()
                                .map_err(|_| format!("invalid option for '{key}'"))?
                                .into(),
                        ),
                        SettingValueType::Number => json!(
                            option
                                .parse::<f64>()
                                .map_err(|_| format!("invalid option for '{key}'"))?
                        ),
                        SettingValueType::Array => {
                            match value.as_array().and_then(|items| items.first()) {
                                Some(Value::Bool(_)) => Value::Bool(option == "true"),
                                Some(Value::Number(number)) if number.as_i64().is_some() => {
                                    Value::Number(
                                        option
                                            .parse::<i64>()
                                            .map_err(|_| format!("invalid option for '{key}'"))?
                                            .into(),
                                    )
                                }
                                Some(Value::Number(_)) => json!(
                                    option
                                        .parse::<f64>()
                                        .map_err(|_| format!("invalid option for '{key}'"))?
                                ),
                                _ => Value::String(option.clone()),
                            }
                        }
                        _ => Value::String(option.clone()),
                    };
                    Ok(SettingOption {
                        value: option_value,
                        label: label
                            .as_str()
                            .ok_or_else(|| format!("option label for '{key}' must be a string"))?
                            .to_owned(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    let display_key = key
        .chars()
        .enumerate()
        .flat_map(|(index, character)| {
            if index > 0 && character.is_uppercase() {
                vec![' ', character]
            } else {
                vec![character]
            }
        })
        .collect::<String>();
    let mut control = metadata
        .get("control")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("setting '{key}' control: {error}"))?;
    if value_type == SettingValueType::Array && options.is_empty() {
        return Err(format!("array setting '{key}' must declare options"));
    }
    if control.is_none() && !options.is_empty() {
        control = Some(if value_type == SettingValueType::Array {
            crate::SettingControl::Multiselect
        } else {
            crate::SettingControl::Select
        });
    }
    Ok(SettingDefinition {
        key: key.into(),
        value_type,
        control,
        label: metadata
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(&display_key)
            .into(),
        description: metadata
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        default: value.clone(),
        group: None,
        minimum: metadata.get("min").and_then(Value::as_f64),
        maximum: metadata.get("max").and_then(Value::as_f64),
        step: metadata.get("step").and_then(Value::as_f64),
        minimum_length: metadata
            .get("minLength")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
        maximum_length: metadata
            .get("maxLength")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
        options,
        restart_required: true,
        apply_at: "restart".into(),
    })
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
    !id.is_empty()
        && id.len() <= 80
        && id != "."
        && id != ".."
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
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
        ).expect("targets are read from extended.mod.json");

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

        infer_api_contract(&mut manifest, "runtime.patch.set_enabled('runtime.patch.example', true)");

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
            .nth(3)
            .expect("package crate lives under src/loader/package");
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
