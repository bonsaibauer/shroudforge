//! One-time conversion of previous ShroudForge layouts. Original files are retained.
use std::{fs, path::Path};
use serde_json::{Value, json};

pub fn migrate_installation(root: &Path) -> Result<Vec<String>, String> {
    crate::config::migrate_state(root)?;
    let mut errors=Vec::new();
    if let Ok(entries)=fs::read_dir(root.join("mods")) {
        for entry in entries.flatten() {
            let package=entry.path();
            if package.is_dir() && package.join("mod.json").is_file() {
                if let Err(error)=migrate_package(root,&package) {errors.push(format!("{}: {error}",package.display()));}
            }
        }
    }
    crate::config::migrate_loader(root)?;
    Ok(errors)
}

pub fn migrate_package(root: &Path, package: &Path) -> Result<(), String> {
    let _lock = crate::config::installation_lock(root)?;
    let path = package.join("mod.json");
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    let mut manifest: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let read = |path: &Path| -> Result<Option<Value>, String> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|error| format!("{}: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    };
    let sidecar = read(&package.join("settings.json"))?;
    let extension_file = read(&package.join("shroudforge.json"))?;
    let id = manifest.get("id").and_then(Value::as_str).ok_or("missing mod id")?.to_owned();
    if !crate::config::valid_id(&id) { return Err("invalid mod id".into()); }
    let config_path = root.join("config/mods").join(format!("{id}.json"));
    let override_values = read(&config_path)?;
    let central = read(&root.join("config/shroudforge.json"))?;
    let central_values = central.as_ref().and_then(|value| value.get("mods")).and_then(|mods| mods.get(&id));
    if override_values.as_ref().zip(central_values).is_some_and(|(a,b)| a != b) {
        return Err(format!("{id}: conflicting settings in legacy central and per-mod files"));
    }
    if manifest.get("shroudforge").is_some() && sidecar.is_none() && extension_file.is_none() {
        let legacy = override_values.as_ref().or(central_values);
        let Some(legacy) = legacy else { return Ok(()); };
        if let Some(enabled) = legacy.get("enabled") { manifest["enabled"] = enabled.clone(); }
        if let Some(settings) = legacy.get("settings") { manifest["settings"] = settings.clone(); }
        crate::config::parse_manifest(root, manifest.clone())?;
        if fs::read(&path).map_err(|e|e.to_string())? != bytes { return Err("mod.json changed during migration".into()); }
        return crate::config::write_json(&path,&manifest);
    }
    if override_values.as_ref().zip(central_values).is_some_and(|(a,b)| a != b) {
        return Err(format!("{id}: conflicting settings in legacy central and per-mod files"));
    }
    let legacy = override_values.as_ref().or(central_values);
    let old_inline = manifest.get("settings").is_some_and(Value::is_array)
        || ["api", "target", "release", "ui", "settingGroups"].iter().any(|key| manifest.get(key).is_some());
    if sidecar.is_none() && extension_file.is_none() && legacy.is_none() && !old_inline { return Ok(()); }
    let definitions = sidecar.as_ref().and_then(|value| value.get("settings"))
        .or_else(|| manifest.get("settings")).and_then(Value::as_array).cloned().unwrap_or_default();
    let mut extension = extension_file.clone()
        .or_else(||manifest.get("shroudforge").cloned())
        .unwrap_or(json!({}));
    extension.as_object_mut().ok_or("mod extension must be an object")?.remove("$schema");
    extension["schemaVersion"] = json!(1);
    for key in ["api", "target", "release", "ui", "settingGroups"] {
        let inline = manifest.as_object_mut().ok_or("manifest must be an object")?.remove(key);
        if extension.get(key).is_none() {
            if let Some(value) = sidecar.as_ref().and_then(|value| value.get(key)).cloned().or(inline) {
                extension[key] = value;
            }
        }
    }
    let mut properties = json!({});
    let mut values = manifest.get("settings").filter(|value| value.is_object()).cloned().unwrap_or(json!({}));
    for definition in definitions {
        let key = definition.get("key").and_then(Value::as_str).ok_or("setting without key")?;
        let mut property = json!({"type": definition["type"], "title": definition["label"],
            "default": definition["default"], "x-ui": definition, "x-apply": "restart"});
        for name in ["minimum", "maximum", "description"] {
            if let Some(value) = definition.get(name) { property[name] = value.clone(); }
        }
        for (old, new) in [("minimumLength", "minLength"), ("maximumLength", "maxLength")] {
            if let Some(value) = definition.get(old) { property[new] = value.clone(); }
        }
        if manifest.get("capabilities").and_then(Value::as_array).is_some_and(|items| items.iter().any(|item| item == "patch" || item == "assets-write")) {
            property["x-apply"] = json!("prepare");
        }
        if let Some(options) = definition.get("options").and_then(Value::as_array).filter(|values| !values.is_empty()) {
            let choices: Vec<_> = options.iter().map(|option| option["value"].clone()).collect();
            if definition["type"] == "array" { property["items"] = json!({"enum": choices}); }
            else { property["enum"] = json!(choices); }
        }
        if values.get(key).is_none() { values[key] = definition["default"].clone(); }
        properties[key] = property;
    }
    if let Some(previous) = legacy.and_then(|value| value.get("settings")).and_then(Value::as_object) {
        for (key, value) in previous { values[key] = value.clone(); }
    }
    manifest["enabled"] = legacy.and_then(|value| value.get("enabled")).cloned()
        .or_else(|| manifest.get("enabled").cloned()).unwrap_or(json!(false));
    manifest["settings"] = values;
    extension["settingsSchema"] = json!({"type":"object", "properties":properties, "additionalProperties":false});
    manifest["shroudforge"] = extension;
    crate::config::parse_manifest(root, manifest.clone())?;
    let backup = package.join(".shroudforge-migration");
    fs::create_dir_all(&backup).map_err(|error| error.to_string())?;
    for name in ["mod.json", "settings.json", "shroudforge.json"] {
        let source = package.join(name);
        let saved = backup.join(name);
        if source.is_file() && !saved.exists() { fs::copy(source, saved).map_err(|error| error.to_string())?; }
    }
    if config_path.is_file() && !backup.join("user-config.json").exists() {
        fs::copy(&config_path, backup.join("user-config.json")).map_err(|error| error.to_string())?;
    }
    if fs::read(&path).map_err(|e| e.to_string())? != bytes { return Err("mod.json changed during migration".into()); }
    crate::config::write_json(&path, &manifest)
}
