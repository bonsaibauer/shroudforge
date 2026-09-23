//! Shared package/configuration boundary for the loader, API and UI.
use crate::{FileSystem, ModManifest, SettingDefinition, validate_manifest};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

pub fn validate_document(root: &Path, name: &str, value: &Value) -> Result<(), String> {
    let embedded = match name {
        "diagnostics-status" => include_str!("../../../config/diagnostics/diagnostics-status-schema.json"),
        "mod" => include_str!("../../../config/mods/mod-schema.json"),
        "rules" => include_str!("../../../config/compatibility/rules-schema.json"),
        "mod-status" => include_str!("../../../config/runtime/mod-status-schema.json"),
        "news" => include_str!("../../../config/news/news-schema.json"),
        "event" => include_str!("../../../config/news/event-schema.json"),
        "news-state" => include_str!("../../../config/news/read-state-schema.json"),
        "events-state" => include_str!("../../../config/news/events-state-schema.json"),
        "catalog-state" => include_str!("../../../config/catalog/state-schema.json"),
        "mod-state" => include_str!("../../../config/mods/state-schema.json"),
        "parser-status" => include_str!("../../../config/runtime/parser-status-schema.json"),
        "api" => include_str!("../../../config/api/api-schema.json"),
        "state" => include_str!("../../../config/updates/state-schema.json"),
        "applied" => include_str!("../../../config/assets/applied-schema.json"),
        "shroudforge" => include_str!("../../../config/loader/shroudforge-schema.json"),
        _ => return Err(format!("unknown schema: {name}")),
    };
    let path = document_path(root, name).with_file_name(format!("{name}-schema.json"));
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => embedded.into(),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let schema: Value =
        serde_json::from_str(&source).map_err(|e| format!("{}: {e}", path.display()))?;
    let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
    validator
        .validate(value)
        .map_err(|e| format!("{name}: {e}"))
}

fn read_package_json(fs: &mut FileSystem, name: &str) -> Result<Value, String> {
    let reader = fs.read_file(name).map_err(|e| format!("{name}: {e}"))?;
    serde_json::from_reader(reader).map_err(|e| format!("{name}: {e}"))
}

/// The same package reader is used by discovery and the UI, for directories and ZIPs.
pub fn read_manifest(root: &Path, fs: &mut FileSystem) -> Result<ModManifest, String> {
    let _ = root;
    parse_manifest(root, read_package_json(fs, "mod.json")?)
}

/// Canonical wire manifest -> internal UI/runtime view. Never serialize this view to disk.
pub fn parse_manifest(root: &Path, mut value: Value) -> Result<ModManifest, String> {
    validate_document(root, "mod", &value)?;
    let extension = value.get("shroudforge").cloned().unwrap_or(json!({}));
    let values = value.get("settings").cloned().unwrap_or(json!({}));
    let mut effective = values.clone();
    let mut definitions = Vec::new();
    if let Some(schema) = extension.get("settingsSchema") {
        jsonschema::validator_for(schema).map_err(|error| error.to_string())?;
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (key, property) in properties {
                if effective.get(key).is_none() {
                    if let Some(default) = property.get("default") { effective[key] = default.clone(); }
                }
                let mut definition = property.get("x-ui").cloned().unwrap_or(json!({}));
                definition["key"] = json!(key);
                definition["type"] = property.get("type").cloned().unwrap_or(json!("string"));
                definition["label"] = property.get("title").cloned().unwrap_or(json!(key));
                definition["default"] = property.get("default").cloned().unwrap_or(Value::Null);
                let apply_at=property.get("x-apply").and_then(Value::as_str)
                    .filter(|value|matches!(*value,"live"|"restart"|"prepare"))
                    .unwrap_or("restart");
                definition["applyAt"] = json!(apply_at);
                for key in ["description", "minimum", "maximum"] {
                    if let Some(entry) = property.get(key) { definition[key] = entry.clone(); }
                }
                for (from, to) in [("minLength", "minimumLength"), ("maxLength", "maximumLength")] {
                    if let Some(entry) = property.get(from) { definition[to] = entry.clone(); }
                }
                definition["restartRequired"] = json!(property.get("x-apply").and_then(Value::as_str) != Some("live"));
                definitions.push(definition);
            }
        }
        jsonschema::validator_for(schema).map_err(|error| error.to_string())?
            .validate(&effective).map_err(|error| format!("settings: {error}"))?;
    } else if !values.as_object().is_some_and(|values| values.is_empty()) {
        return Err("settings require shroudforge.settingsSchema".into());
    }
    let object = value.as_object_mut().ok_or("mod.json must be an object")?;
    object.remove("$schema");
    object.remove("shroudforge");
    object.insert("settingValues".into(), effective);
    object.insert("settings".into(), json!(definitions));
    for key in ["api", "target", "release", "settingGroups", "ui"] {
        if let Some(entry) = extension.get(key) { object.insert(key.into(), entry.clone()); }
    }
    object.insert("settings_schema".into(), extension.get("settingsSchema").cloned().unwrap_or(Value::Null));
    let manifest: ModManifest = serde_json::from_value(value).map_err(|error| error.to_string())?;
    validate_manifest(&manifest)?;
    Ok(manifest)
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

fn package_for_id(root: &Path, id: &str) -> Result<std::path::PathBuf, String> {
    if !valid_id(id) { return Err("invalid mod id".into()); }
    let mut result = None;
    for entry in fs::read_dir(root.join("mods")).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let Ok(manifest) = read_manifest_path(root, &path) else { continue };
        if manifest.id == id {
            if result.is_some() { return Err(format!("duplicate mod id: {id}")); }
            result = Some(path);
        }
    }
    result.ok_or_else(|| format!("mod not found: {id}"))
}

pub fn read_mod_config(root: &Path, id: &str) -> Result<Value, String> {
    let package = package_for_id(root, id)?;
    let manifest = read_manifest_path(root, &package)?;
    Ok(json!({"enabled": manifest.enabled, "settings": manifest.setting_values}))
}

/// Preserve explicit user choices while validating them against the incoming contract.
/// Incompatible or removed settings abort the update so the old installation can be kept.
pub fn merge_mod_update(root: &Path, previous: Value, mut incoming: Value) -> Result<Value, String> {
    let old = parse_manifest(root, previous)?;
    let new = parse_manifest(root, incoming.clone())?;
    if old.id != new.id { return Err("mod update changes the package id".into()); }
    incoming["enabled"] = json!(old.enabled);
    let mut settings = new.setting_values;
    for (key, value) in old.setting_values.as_object().ok_or("settings must be an object")? {
        settings[key] = value.clone();
    }
    incoming["settings"] = settings;
    parse_manifest(root, incoming.clone()).map_err(|error| format!("mod settings require migration: {error}"))?;
    Ok(incoming)
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
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn write_document(root: &Path, name: &str, value: &Value) -> Result<(), String> {
    validate_document(root, name, value)?;
    if let Some(section) = state_section(name) {
        return update_state_section(root, section, |_| Ok(value.clone())).and_then(|_| Ok(()));
    }
    write_json(&document_path(root, name), value)
}

fn state_section(name: &str) -> Option<&'static str> {
    match name { "state" => Some("updates"), "applied" => Some("assets"), "mod-status" => Some("runtime"), "diagnostics-status" => Some("diagnostics"), "news-state" => Some("news"), "events-state" => Some("events"), "catalog-state" => Some("catalog"), "mod-state" => Some("mods"), "parser-status" => Some("parser"), _ => None }
}

fn read_state_file(root: &Path) -> Result<Value, String> {
    match fs::read(root.join("config/state.json")) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes).map_err(|error| format!("config/state.json: {error}"))?;
            let schema: Value = serde_json::from_str(include_str!("../../../config/state-schema.json")).map_err(|error| error.to_string())?;
            jsonschema::validator_for(&schema).map_err(|error| error.to_string())?.validate(&value).map_err(|error| format!("config/state.json: {error}"))?;
            Ok(value)
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({"schemaVersion":1})),
        Err(error) => Err(error.to_string()),
    }
}

pub fn read_document(root: &Path, name: &str) -> Result<Value, String> {
    if let Some(section) = state_section(name) {
        let state = read_state_file(root)?;
        let Some(value) = state.get(section) else { return Err(format!("config/state.json has no {section} state")); };
        validate_document(root, name, value)?;
        return Ok(value.clone());
    }
    let path = document_path(root, name);
    let value: Value = serde_json::from_slice(&fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    validate_document(root, name, &value)?;
    Ok(value)
}

pub fn update_state_section(root: &Path, section: &str, update: impl FnOnce(Option<&Value>) -> Result<Value, String>) -> Result<(), String> {
    let _lock = installation_lock(root)?;
    let mut state = read_state_file(root)?;
    if state.get("schemaVersion").and_then(Value::as_u64).is_some_and(|version| version != 1) {
        return Err("unsupported config/state.json schemaVersion".into());
    }
    state["schemaVersion"] = json!(1);
    let value = update(state.get(section))?;
    state[section] = value;
    write_json(&root.join("config/state.json"), &state)
}

/// Import prior per-feature state files once, under the same installation-wide lock.
/// Legacy files remain untouched as recovery copies.
pub fn migrate_state(root: &Path) -> Result<(), String> {
    let _lock = installation_lock(root)?;
    let mut state = read_state_file(root)?;
    let legacy = [
        ("updates", "config/updates/state.json", "state"),
        ("assets", "config/assets/applied.json", "applied"),
        ("runtime", "config/runtime/mod-status.json", "mod-status"),
        ("diagnostics", "config/diagnostics/diagnostics-status.json", "diagnostics-status"),
        ("news", "config/news/read-state.json", "news-state"),
        ("catalog", "Shroudforge_UI/catalog-installs.json", "catalog-state"),
        ("mods", "config/news/mod-state.json", "mod-state"),
    ];
    let mut changed = false;
    for (section, path, schema) in legacy {
        if state.get(section).is_some() { continue; }
        let path = root.join(path);
        let Ok(bytes) = fs::read(&path) else { continue; };
        let mut value: Value = serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
        if section == "news" && value.get("readAt").is_none() {
            let mut read_at = serde_json::Map::new();
            let timestamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            if let Some(ids) = value.get("read").and_then(Value::as_array) {
                for id in ids.iter().filter_map(Value::as_str) { read_at.insert(id.into(), json!(timestamp)); }
            }
            value["readAt"] = Value::Object(read_at);
        }
        validate_document(root, schema, &value).map_err(|error| format!("{}: {error}", path.display()))?;
        state[section] = value;
        changed = true;
    }
    if state.get("news").is_none() {
        let path=root.join("Shroudforge_UI/news-state.json");
        if let Ok(bytes)=fs::read(&path) {
            let mut value:Value=serde_json::from_slice(&bytes).map_err(|error|format!("{}: {error}",path.display()))?;
            if value.get("readAt").is_none() {
                let timestamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                let mut read_at=serde_json::Map::new();
                if let Some(ids)=value.get("read").and_then(Value::as_array){for id in ids.iter().filter_map(Value::as_str){read_at.insert(id.into(),json!(timestamp));}}
                value["readAt"]=Value::Object(read_at);
            }
            validate_document(root,"news-state",&value)?;
            state["news"]=value;changed=true;
        }
    }
    if state.get("events").is_none() {
        let directory=root.join("config/news/events");
        if let Ok(entries)=fs::read_dir(directory) {
            let mut events=serde_json::Map::new();
            for entry in entries.flatten() {
                let path=entry.path();
                if path.extension().is_none_or(|ext|ext!="json"){continue;}
                let bytes=fs::read(&path).map_err(|error|error.to_string())?;
                let value:Value=serde_json::from_slice(&bytes).map_err(|error|format!("{}: {error}",path.display()))?;
                validate_document(root,"event",&value)?;
                events.insert(entry.file_name().to_string_lossy().into_owned(),value);
            }
            if !events.is_empty(){state["events"]=Value::Object(events);changed=true;}
        }
    }
    if state.get("mods").is_none() {
        let path=root.join("Shroudforge_UI/mod-state.json");
        if let Ok(bytes)=fs::read(&path){let value:Value=serde_json::from_slice(&bytes).map_err(|error|error.to_string())?;validate_document(root,"mod-state",&value)?;state["mods"]=value;changed=true;}
    }
    if changed {
        state["schemaVersion"] = json!(1);
        write_json(&root.join("config/state.json"), &state)?;
    }
    Ok(())
}

pub fn document_path(root: &Path, name: &str) -> std::path::PathBuf {
    match name {
        "shroudforge" => root.join("config/shroudforge.json"),
        "state" | "applied" | "mod-status" | "diagnostics-status" | "news-state" | "events-state" | "catalog-state" | "mod-state" | "parser-status" => root.join("config/state.json"),
        "news" => root.join("config/news/news.json"),
        other => root.join("config").join(match other {
            "diagnostics-status" => "diagnostics",
            "mod" => "mods",
            "mod-status" => "runtime",
            "rules" => "compatibility",
            "applied" => "assets",
            value => value,
        }).join(format!("{other}.json")),
    }
}

pub fn read_loader(root: &Path) -> Result<Value, String> {
    let path = document_path(root, "shroudforge");
    let defaults: Value = serde_json::from_str(include_str!("../../../config/shroudforge.json")).map_err(|error| error.to_string())?;
    let mut result = defaults;
    let old_path = root.join("config/loader/shroudforge.json");
    let loaded = match fs::read(if path.exists() { &path } else { &old_path }) {
        Ok(bytes) => Some(serde_json::from_slice::<Value>(&bytes).map_err(|error| error.to_string())?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    fn merge(target: &mut Value, source: Value) {
        match (target, source) {
            (Value::Object(target), Value::Object(source)) => for (key, value) in source { merge(target.entry(key).or_insert(Value::Null), value); },
            (target, source) => *target = source,
        }
    }
    if let Some(mut loaded) = loaded {
        let old_layout = !path.exists();
        if old_layout {
            let object = loaded.as_object_mut().ok_or("legacy loader configuration must be an object")?;
            object.remove("mods");
            if let Some(logging) = object.get_mut("logging").and_then(Value::as_object_mut) {
                if let Some(level) = logging.remove("level") { logging.entry("minimumLevel").or_insert(level); }
            }
        }
        if let Some(object) = loaded.as_object_mut() { object.remove("mods"); }
        merge(&mut result, loaded);
    }
    validate_document(root, "shroudforge", &result)?;
    Ok(result)
}

pub fn migrate_loader(root: &Path) -> Result<(), String> {
    let _lock = installation_lock(root)?;
    let path = document_path(root, "shroudforge");
    let legacy = root.join("config/loader/shroudforge.json");
    let source = if path.is_file() { &path } else { &legacy };
    let bytes = match fs::read(source) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let mut value: Value = serde_json::from_slice(&bytes).map_err(|error|error.to_string())?;
    let object=value.as_object_mut().ok_or("loader configuration must be an object")?;
    object.remove("mods");
    if let Some(logging)=object.get_mut("logging").and_then(Value::as_object_mut) {
        if let Some(level)=logging.remove("level") {logging.entry("minimumLevel").or_insert(level);}
    }
    validate_document(root,"shroudforge",&value)?;
    let normalized=serde_json::to_vec_pretty(&value).map_err(|error|error.to_string())?;
    if source != &path || bytes!=normalized {write_json(&path,&value)?;}
    Ok(())
}

pub fn update_loader(root: &Path, update: impl FnOnce(&mut Value) -> Result<(), String>) -> Result<(), String> {
    let path = document_path(root, "shroudforge");
    fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
    let _lock = installation_lock(root)?;
    let mut value = read_loader(root)?;
    update(&mut value)?;
    validate_document(root, "shroudforge", &value)?;
    write_json(&path, &value)
}

pub fn installation_lock(root: &Path) -> Result<fs::File, String> {
    fs::create_dir_all(root.join("config")).map_err(|error| error.to_string())?;
    let lock = fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
        .open(root.join("config/.shroudforge-write.lock")).map_err(|error| error.to_string())?;
    lock.lock_exclusive().map_err(|error| error.to_string())?;
    Ok(lock)
}

pub fn update_mod_config(
    root: &Path,
    id: &str,
    update: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    update_mod_config_revision(root, id, None, update)
}

pub fn revision(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod update_tests {
    use super::*;
    fn manifest(default: i64, maximum: i64) -> Value {
        json!({"id":"example","name":"Example","version":"1.0.0","enabled":false,"settings":{"count":default},
            "shroudforge":{"schemaVersion":1,"settingsSchema":{"type":"object","properties":{"count":{"type":"integer","default":default,"minimum":0,"maximum":maximum}},"additionalProperties":false}}})
    }
    #[test]
    fn updates_preserve_values_and_reject_incompatible_new_definitions() {
        let root=tempfile::tempdir().unwrap();
        let mut old=manifest(7,10); old["enabled"]=json!(true);
        let merged=merge_mod_update(root.path(),old.clone(),manifest(2,20)).unwrap();
        assert_eq!(merged["settings"]["count"],7); assert_eq!(merged["enabled"],true);
        assert!(merge_mod_update(root.path(),old,manifest(2,5)).is_err());
    }
    #[test]
    fn stale_editor_cannot_overwrite_a_manual_change() {
        let root=tempfile::tempdir().unwrap();
        let path=root.path().join("mods/example/mod.json");
        write_json(&path,&manifest(3,10)).unwrap();
        let displayed=revision(&fs::read(&path).unwrap());
        write_json(&path,&manifest(4,10)).unwrap();
        assert!(update_mod_config_revision(root.path(),"example",Some(&displayed),|value|{value["settings"]["count"]=json!(8);Ok(())}).is_err());
        let current:Value=serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(current["settings"]["count"],4);
    }
}

pub fn update_mod_config_revision(
    root: &Path,
    id: &str,
    expected: Option<&str>,
    update: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let package = package_for_id(root, id)?;
    if !package.is_dir() { return Err("install the ZIP as a directory before editing its manifest".into()); }
    let path = package.join("mod.json");
    let _lock = installation_lock(root)?;
    let before = fs::read(&path).map_err(|error| error.to_string())?;
    if expected.is_some_and(|expected| expected != revision(&before)) {
        return Err("mod.json changed since it was displayed; reload before saving".into());
    }
    let mut value: Value = serde_json::from_slice(&before).map_err(|error| error.to_string())?;
    update(&mut value)?;
    parse_manifest(root, value.clone())?;
    if fs::read(&path).map_err(|error| error.to_string())? != before {
        return Err("mod.json changed during editing; reload before saving".into());
    }
    write_json(&path, &value)
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

pub fn effective_settings(_root: &Path, manifest: &ModManifest) -> Result<Value, String> {
    Ok(manifest.setting_values.clone())
}
