//! Shared package/configuration boundary for the loader, API and UI.
use crate::SettingDefinition;
use fs2::FileExt;

pub use crate::registry::{
    infer_api_contract, parse_manifest, parse_manifest_with_extension, read_manifest,
    read_manifest_path, valid_id,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

pub fn validate_document(_root: &Path, name: &str, value: &Value) -> Result<(), String> {
    let embedded = match name {
        "diagnostics-status" => {
            include_str!("../../modules/runtime-diagnostics/diagnostics-status.schema.json")
        }
        "mod" => include_str!("registry/manifest.schema.json"),
        "extended-mod" => include_str!("registry/extended.mod.schema.json"),
        "rules" => include_str!("compatibility/rules.schema.json"),
        "mod-status" => include_str!("status/runtime-mod.schema.json"),
        "news" => include_str!("news/news-schema.json"),
        "event" => include_str!("news/event-schema.json"),
        "news-state" => include_str!("news/read-state-schema.json"),
        "events-state" => include_str!("news/events-state-schema.json"),
        "catalog-state" => include_str!("status/catalog.schema.json"),
        "mod-state" => include_str!("status/mods.schema.json"),
        "parser-status" => include_str!("../../../parser/parser-status.schema.json"),
        "window-state" => include_str!("status/windows.schema.json"),
        "api" => include_str!("../../api/src/shroudforge/v1/runtime-operations.schema.json"),
        "state" => include_str!("status/loader.schema.json"),
        "applied" => include_str!("prepared.schema.json"),
        "shroudforge" => include_str!("config/loader.schema.json"),
        _ => return Err(format!("unknown schema: {name}")),
    };
    let schema: Value =
        serde_json::from_str(embedded).map_err(|e| format!("schema for {name}: {e}"))?;
    let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
    validator
        .validate(value)
        .map_err(|e| format!("{name}: {e}"))
}

/// Atomic replacement; readers see either the old or the complete new document.
pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path.parent().ok_or("missing config parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary
        .write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    let mut temporary = temporary;
    for attempt in 0..5 {
        match temporary.persist(path) {
            Ok(_) => return Ok(()),
            Err(error)
                if error.error.kind() == std::io::ErrorKind::PermissionDenied && attempt < 4 =>
            {
                temporary = error.file;
                std::thread::sleep(std::time::Duration::from_millis(10 << attempt));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("state file replacement retries exhausted".into())
}

pub fn write_document(root: &Path, name: &str, value: &Value) -> Result<(), String> {
    if name == "mod" {
        return Err("mod.json is package data; edit the mod package instead".into());
    }
    validate_document(root, name, value)?;
    if let Some(section) = state_section(name) {
        return update_state_section(root, section, |_| Ok(value.clone())).and_then(|_| Ok(()));
    }
    write_json(&document_path(root, name), value)
}

fn state_section(name: &str) -> Option<&'static str> {
    match name {
        "state" => Some("updates"),
        "applied" => Some("assets"),
        "mod-status" => Some("runtime"),
        "diagnostics-status" => Some("diagnostics"),
        "window-state" => Some("windows"),
        "news-state" => Some("news"),
        "events-state" => Some("events"),
        "catalog-state" => Some("catalog"),
        "mod-state" => Some("mods"),
        "parser-status" => Some("parser"),
        _ => None,
    }
}

fn read_state_file(root: &Path) -> Result<Value, String> {
    let state_path = root.join("shroudforge/state/state.json");
    match fs::read(&state_path) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("shroudforge/state/state.json: {error}"))?;
            let schema: Value = serde_json::from_str(include_str!("status/loader.schema.json"))
                .map_err(|error| error.to_string())?;
            jsonschema::validator_for(&schema)
                .map_err(|error| error.to_string())?
                .validate(&value)
                .map_err(|error| format!("shroudforge/state/state.json: {error}"))?;
            Ok(value)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(json!({"schemaVersion":1}))
        }
        Err(error) => Err(error.to_string()),
    }
}

pub fn read_document(root: &Path, name: &str) -> Result<Value, String> {
    if name == "mod" {
        return Err("mod.json is package data; read it from the mod package instead".into());
    }
    if let Some(section) = state_section(name) {
        let state = read_state_file(root)?;
        let Some(value) = state.get(section) else {
            return Err(format!(
                "shroudforge/state/state.json has no {section} state"
            ));
        };
        validate_document(root, name, value)?;
        return Ok(value.clone());
    }
    let path = document_path(root, name);
    let value: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{}: {error}", path.display()))?;
    validate_document(root, name, &value)?;
    Ok(value)
}

pub fn update_state_section(
    root: &Path,
    section: &str,
    update: impl FnOnce(Option<&Value>) -> Result<Value, String>,
) -> Result<(), String> {
    let _lock = installation_lock(root)?;
    let mut state = read_state_file(root)?;
    if state
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .is_some_and(|version| version != 1)
    {
        return Err("unsupported shroudforge/state/state.json schemaVersion".into());
    }
    state["schemaVersion"] = json!(1);
    let value = update(state.get(section))?;
    state[section] = value;
    write_json(&root.join("shroudforge/state/state.json"), &state)
}

pub fn window_state(root: &Path) -> Value {
    read_state_file(root)
        .ok()
        .and_then(|state| state.get("windows").cloned())
        .unwrap_or_else(|| json!({}))
}

pub fn request_window_visibility(root: &Path, module: &str, visible: bool) -> Result<(), String> {
    if !matches!(module, "modloaderUi" | "debugConsole") {
        return Err("unknown window module".into());
    }
    update_state_section(root, "windows", |existing| {
        let mut state = existing.cloned().unwrap_or_else(|| json!({}));
        let current = state.get(module).cloned().unwrap_or_else(|| json!({}));
        let request_id = current
            .get("requestId")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .saturating_add(1);
        state[module] = json!({"visible":current.get("visible").and_then(Value::as_bool).unwrap_or(false),
            "requestedVisible":visible,"requestId":request_id});
        Ok(state)
    })
}

pub fn publish_window_visibility(root: &Path, module: &str, visible: bool) -> Result<(), String> {
    if !matches!(module, "modloaderUi" | "debugConsole") {
        return Err("unknown window module".into());
    }
    update_state_section(root, "windows", |existing| {
        let mut state = existing.cloned().unwrap_or_else(|| json!({}));
        let current = state.get(module).cloned().unwrap_or_else(|| json!({}));
        state[module] = json!({"visible":visible,
            "requestedVisible":current.get("requestedVisible").and_then(Value::as_bool).unwrap_or(visible),
            "requestId":current.get("requestId").and_then(Value::as_u64).unwrap_or(0)});
        Ok(state)
    })
}

pub fn document_path(root: &Path, name: &str) -> std::path::PathBuf {
    match name {
        "shroudforge" => crate::paths::config_dir(root).join("loader.json"),
        "state" | "applied" | "mod-status" | "diagnostics-status" | "window-state"
        | "news-state" | "events-state" | "catalog-state" | "mod-state" | "parser-status" => {
            root.join("shroudforge/state/state.json")
        }
        "news" => crate::paths::config_dir(root).join("news/news.json"),
        other => crate::paths::config_dir(root)
            .join(match other {
                "diagnostics-status" => "diagnostics",
                "window-state" => "windows",
                "mod-status" => "runtime",
                "rules" => "compatibility",
                "applied" => "assets",
                value => value,
            })
            .join(format!("{other}.json")),
    }
}

pub fn read_loader(root: &Path) -> Result<Value, String> {
    let path = document_path(root, "shroudforge");
    let defaults: Value = serde_json::from_str(include_str!("config/loader.default.json"))
        .map_err(|error| error.to_string())?;
    let mut result = defaults;
    let loaded = match fs::read(&path) {
        Ok(bytes) => {
            Some(serde_json::from_slice::<Value>(&bytes).map_err(|error| error.to_string())?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
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
    validate_document(root, "shroudforge", &result)?;
    Ok(result)
}

pub fn update_loader(
    root: &Path,
    update: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let path = document_path(root, "shroudforge");
    fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
    let _lock = installation_lock(root)?;
    let mut value = read_loader(root)?;
    update(&mut value)?;
    validate_document(root, "shroudforge", &value)?;
    write_json(&path, &value)
}

/// Import EML's proxy configuration once into the ShroudForge loader config.
/// The old file is left untouched as a reference and is not consulted again
/// after the import marker has been written.
pub fn migrate_eml_config_once(root: &Path) -> Result<(), String> {
    update_loader(root, |loader| {
        if let Some(logging) = loader
            .get_mut("logging")
            .and_then(Value::as_object_mut)
        {
            logging.remove("nativeConsole");
        }
        if loader
            .pointer("/migrations/emlJsonV1")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(());
        }

        let legacy_path = root.join("eml.json");
        if legacy_path.is_file() {
            let bytes = fs::read(&legacy_path).map_err(|error| error.to_string())?;
            let legacy: Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", legacy_path.display()))?;

            if let Some(enabled) = legacy.get("use_export_flag").and_then(Value::as_bool) {
                loader["exports"]["enabled"] = json!(enabled);
            }
            if let Some(directory) = legacy
                .get("export_directory")
                .and_then(Value::as_str)
                .filter(|directory| valid_export_directory(directory))
            {
                loader["exports"]["directory"] = json!(directory);
            }
        }

        loader["migrations"]["emlJsonV1"] = json!(true);
        Ok(())
    })
}

pub fn valid_export_directory(directory: &str) -> bool {
    use std::path::Component;
    !directory.trim().is_empty()
        && !Path::new(directory).is_absolute()
        && Path::new(directory)
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

pub fn installation_lock(root: &Path) -> Result<fs::File, String> {
    fs::create_dir_all(crate::paths::config_dir(root)).map_err(|error| error.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(crate::paths::config_dir(root).join(".shroudforge-write.lock"))
        .map_err(|error| error.to_string())?;
    lock.lock_exclusive().map_err(|error| error.to_string())?;
    Ok(lock)
}

pub fn revision(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// Fingerprint the two files that define package metadata and user settings.
/// This works for both directory and ZIP packages without parsing every Lua
/// source file on each runtime status refresh.
pub fn manifest_revision(package: &Path) -> Result<String, String> {
    let path = camino::Utf8Path::from_path(package).ok_or("package path is not UTF-8")?;
    let mut package_fs = if package.is_dir() {
        crate::FileSystem::new_disk(path)
    } else {
        crate::FileSystem::new_zip(path)
    }
    .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    for name in ["mod.json", "extended.mod.json"] {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        match package_fs.read_file(name) {
            Ok(mut file) => {
                file.read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => bytes.push(0xff),
            Err(error) => return Err(format!("{}: {error}", package.display())),
        }
        bytes.push(0);
    }
    Ok(revision(&bytes))
}

pub fn validate_settings(definitions: &[SettingDefinition], values: &Value) -> Result<(), String> {
    let object = values.as_object().ok_or("settings must be an object")?;
    for (key, value) in object {
        let definition = definitions
            .iter()
            .find(|d| d.key == *key)
            .ok_or_else(|| format!("unknown setting: {key}"))?;
        let mut schema = json!({"type": definition.value_type});
        if let Some(v) = definition.minimum {
            schema["minimum"] = json!(v);
        }
        if let Some(v) = definition.maximum {
            schema["maximum"] = json!(v);
        }
        if let Some(v) = definition.minimum_length {
            schema["minLength"] = json!(v);
        }
        if let Some(v) = definition.maximum_length {
            schema["maxLength"] = json!(v);
        }
        if !definition.options.is_empty() {
            let options: Vec<_> = definition.options.iter().map(|v| v.value.clone()).collect();
            if value.is_array() {
                schema["items"] = json!({"enum": options});
            } else {
                schema["enum"] = json!(options);
            }
        }
        let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
        validator
            .validate(value)
            .map_err(|e| format!("{key}: {e}"))?;
    }
    Ok(())
}
