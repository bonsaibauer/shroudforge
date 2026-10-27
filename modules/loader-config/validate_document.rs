use crate::Document;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, LazyLock, OnceLock, RwLock},
};

type CachedValidator = Arc<OnceLock<Result<jsonschema::Validator, String>>>;

static VALIDATORS: LazyLock<RwLock<HashMap<(&'static str, &'static str), CachedValidator>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn validate_document(_root: &Path, document: Document, value: &Value) -> Result<(), String> {
    let name = document.name;
    if name == crate::documents::LOADER.name {
        let mut protected_scancodes = HashSet::new();
        if let Some(keys) = value
            .pointer("/hotkeys/protectedKeys")
            .and_then(Value::as_array)
        {
            for key in keys {
                if let Some(scancode) = key.get("scancode").and_then(Value::as_u64) {
                    shroudforge_hotkeys::parse_hotkey(&format!("SDL:{scancode}"))
                        .map_err(|error| format!("hotkeys.protectedKeys: {error}"))?;
                    if !protected_scancodes.insert(scancode) {
                        return Err(format!(
                            "shroudforge: duplicate protected SDL scancode {scancode}"
                        ));
                    }
                }
            }
        }
        if let Some(modules) = value.get("modules").and_then(Value::as_object) {
            for (module, settings) in modules {
                if let Some(binding) = settings.get("toggleKey") {
                    shroudforge_hotkeys::parse_value(binding)
                        .map_err(|error| format!("{module}.toggleKey: {error}"))?;
                }
            }
        }
    }
    // Documents contain immutable, embedded schemas. Reuse their compilation,
    // while validating the current value on every read and write as before.
    let key = (name, document.schema);
    let cached = {
        VALIDATORS
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            .cloned()
    };
    let cached = cached.unwrap_or_else(|| {
        VALIDATORS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(key)
            .or_insert_with(|| Arc::new(OnceLock::new()))
            .clone()
    });
    // Compilation and validation run without holding the shared cache lock.
    let validator = cached
        .get_or_init(|| {
            let schema: Value = serde_json::from_str(document.schema)
                .map_err(|e| format!("schema for {name}: {e}"))?;
            jsonschema::validator_for(&schema).map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    validator
        .validate(value)
        .map_err(|e| format!("{name}: {e}"))
}
