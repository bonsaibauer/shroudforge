//! Shared package/configuration boundary for the loader, API and UI.
use crate::{FileSystem, ModManifest, SettingDefinition, validate_manifest};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{fs, io::{Read, Write}, path::Path};

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
        "window-state" => include_str!("../../../config/window-state-schema.json"),
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
    let mut manifest = parse_manifest(root, read_package_json(fs, "mod.json")?)?;
    let mut source = String::new();
    collect_lua_sources(fs, camino::Utf8Path::new("src"), &mut source)?;
    infer_api_contract(&mut manifest, &source);
    Ok(manifest)
}

fn collect_lua_sources(fs: &mut FileSystem, directory: &camino::Utf8Path, output: &mut String) -> Result<(), String> {
    let entries = match fs.read_directory(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    for entry in entries.into_iter().flatten() {
        if fs.is_directory(&entry) {
            collect_lua_sources(fs, &entry, output)?;
        } else if entry.extension() == Some("lua") {
            let Ok(mut file) = fs.read_file(&entry) else { continue; };
            file.read_to_string(output).map_err(|error| error.to_string())?;
            output.push('\n');
        }
    }
    Ok(())
}

/// Derive execution phase and process scope from the Lua entrypoint's API usage.
/// These values are runtime metadata and are not author-maintained manifest flags.
pub fn infer_api_contract(manifest: &mut ModManifest, source: &str) {
    let normalized: String = strip_lua_comments(source).chars().filter(|character| !character.is_whitespace()).collect();
    let assets = normalized.contains("runtime.require(\"game.assets.write\")")
        || normalized.contains("runtime.require('game.assets.write')")
        || normalized.contains("game.assets.");
    let runtime = normalized.contains("runtime.require(\"runtime.lifecycle\")")
        || normalized.contains("runtime.require('runtime.lifecycle')")
        || normalized.contains("runtime.ecs.")
        || normalized.contains("on_load=")
        || normalized.contains("on_update=")
        || normalized.contains("on_unload=");
    let export = normalized.contains("io.export(") || normalized.contains("io.export");
    manifest.capabilities.clear();
    if assets { manifest.capabilities.push(crate::Capability::AssetsWrite); }
    if runtime { manifest.capabilities.push(crate::Capability::Runtime); }
    if export { manifest.capabilities.push(crate::Capability::Export); }
    manifest.target = if runtime && !assets && !export { crate::ModTarget::Client } else { crate::ModTarget::Both };
    manifest.api = None;
    let apply_at = if runtime && !assets { "live" } else { "restart" };
    for setting in &mut manifest.settings {
        setting.apply_at = apply_at.into();
        setting.restart_required = apply_at != "live";
    }
}

fn strip_lua_comments(source: &str) -> String {
    let mut output=String::with_capacity(source.len());
    let mut chars=source.chars().peekable();
    let mut quote=None;
    let mut escaped=false;
    let mut line_comment=false;
    let mut block_comment=false;
    while let Some(character)=chars.next() {
        if line_comment {
            if character=='\n' { line_comment=false; output.push(character); }
            continue;
        }
        if block_comment {
            if character==']' && chars.peek()==Some(&']') { chars.next(); block_comment=false; }
            continue;
        }
        if let Some(delimiter)=quote {
            output.push(character);
            if escaped { escaped=false; continue; }
            if character=='\\' { escaped=true; continue; }
            if character==delimiter { quote=None; }
            continue;
        }
        if character=='\'' || character=='"' { quote=Some(character); output.push(character); continue; }
        if character=='-' && chars.peek()==Some(&'-') {
            chars.next();
            if chars.peek()==Some(&'[') {
                chars.next();
                if chars.peek()==Some(&'[') { chars.next(); block_comment=true; continue; }
                output.push_str("--[");
                continue;
            }
            line_comment=true;
            continue;
        }
        output.push(character);
    }
    output
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
                if let Some(definition)=definition.as_object_mut() {
                    definition.remove("applyAt");
                    definition.remove("restartRequired");
                }
                definition["key"] = json!(key);
                definition["type"] = property.get("type").cloned().unwrap_or(json!("string"));
                definition["label"] = property.get("title").cloned().unwrap_or(json!(key));
                definition["default"] = property.get("default").cloned().unwrap_or(Value::Null);
                let apply_at="restart";
                definition["applyAt"] = json!(apply_at);
                for key in ["description", "minimum", "maximum"] {
                    if let Some(entry) = property.get(key) { definition[key] = entry.clone(); }
                }
                for (from, to) in [("minLength", "minimumLength"), ("maxLength", "maximumLength")] {
                    if let Some(entry) = property.get(from) { definition[to] = entry.clone(); }
                }
                definition["restartRequired"] = json!(true);
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
    for key in ["release", "links", "settingGroups", "ui"] {
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
    parse_manifest(root, incoming.clone()).map_err(|error| format!("updated mod manifest is incompatible with the current schema: {error}"))?;
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
    let mut temporary = temporary;
    for attempt in 0..5 {
        match temporary.persist(path) {
            Ok(_) => return Ok(()),
            Err(error) if error.error.kind() == std::io::ErrorKind::PermissionDenied && attempt < 4 => {
                temporary = error.file;
                std::thread::sleep(std::time::Duration::from_millis(10 << attempt));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("state file replacement retries exhausted".into())
}

pub fn write_document(root: &Path, name: &str, value: &Value) -> Result<(), String> {
    validate_document(root, name, value)?;
    if let Some(section) = state_section(name) {
        return update_state_section(root, section, |_| Ok(value.clone())).and_then(|_| Ok(()));
    }
    write_json(&document_path(root, name), value)
}

fn state_section(name: &str) -> Option<&'static str> {
    match name { "state" => Some("updates"), "applied" => Some("assets"), "mod-status" => Some("runtime"), "diagnostics-status" => Some("diagnostics"), "window-state" => Some("windows"), "news-state" => Some("news"), "events-state" => Some("events"), "catalog-state" => Some("catalog"), "mod-state" => Some("mods"), "parser-status" => Some("parser"), _ => None }
}

fn read_state_file(root: &Path) -> Result<Value, String> {
    match fs::read(crate::paths::config_dir(root).join("state.json")) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes).map_err(|error| format!("shroudforge/config/state.json: {error}"))?;
            let schema: Value = serde_json::from_str(include_str!("../../../config/state-schema.json")).map_err(|error| error.to_string())?;
            jsonschema::validator_for(&schema).map_err(|error| error.to_string())?.validate(&value).map_err(|error| format!("shroudforge/config/state.json: {error}"))?;
            Ok(value)
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({"schemaVersion":1})),
        Err(error) => Err(error.to_string()),
    }
}

pub fn read_document(root: &Path, name: &str) -> Result<Value, String> {
    if let Some(section) = state_section(name) {
        let state = read_state_file(root)?;
        let Some(value) = state.get(section) else { return Err(format!("shroudforge/config/state.json has no {section} state")); };
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
        return Err("unsupported shroudforge/config/state.json schemaVersion".into());
    }
    state["schemaVersion"] = json!(1);
    let value = update(state.get(section))?;
    state[section] = value;
    write_json(&crate::paths::config_dir(root).join("state.json"), &state)
}

pub fn window_state(root: &Path) -> Value {
    read_state_file(root).ok().and_then(|state| state.get("windows").cloned()).unwrap_or_else(|| json!({}))
}

pub fn request_window_visibility(root: &Path, module: &str, visible: bool) -> Result<(), String> {
    if !matches!(module, "modloaderUi" | "debugConsole") { return Err("unknown window module".into()); }
    update_state_section(root, "windows", |existing| {
        let mut state = existing.cloned().unwrap_or_else(|| json!({}));
        let current = state.get(module).cloned().unwrap_or_else(|| json!({}));
        let request_id = current.get("requestId").and_then(Value::as_u64).unwrap_or(0).saturating_add(1);
        state[module] = json!({"visible":current.get("visible").and_then(Value::as_bool).unwrap_or(false),
            "requestedVisible":visible,"requestId":request_id});
        Ok(state)
    })
}

pub fn publish_window_visibility(root: &Path, module: &str, visible: bool) -> Result<(), String> {
    if !matches!(module, "modloaderUi" | "debugConsole") { return Err("unknown window module".into()); }
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
        "shroudforge" => crate::paths::config_dir(root).join("shroudforge.json"),
        "state" | "applied" | "mod-status" | "diagnostics-status" | "window-state" | "news-state" | "events-state" | "catalog-state" | "mod-state" | "parser-status" => crate::paths::config_dir(root).join("state.json"),
        "news" => crate::paths::config_dir(root).join("news/news.json"),
        other => crate::paths::config_dir(root).join(match other {
            "diagnostics-status" => "diagnostics",
            "window-state" => "windows",
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
    let loaded = match fs::read(&path) {
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
    if let Some(loaded) = loaded {
        merge(&mut result, loaded);
    }
    validate_document(root, "shroudforge", &result)?;
    Ok(result)
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
    fs::create_dir_all(crate::paths::config_dir(root)).map_err(|error| error.to_string())?;
    let lock = fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
        .open(crate::paths::config_dir(root).join(".shroudforge-write.lock")).map_err(|error| error.to_string())?;
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
