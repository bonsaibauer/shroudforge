use crate::config::validate_settings;
use crate::read_mod_settings::parse_setting_definition;
use crate::{ModManifest, SettingGroup, SettingPreset, validate_manifest};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

const MOD_FIELDS: &[&str] = &[
    "$schema",
    "id",
    "name",
    "version",
    "description",
    "authors",
    "license",
    "icon",
    "dependencies",
    "capabilities",
];

const EXTENDED_MOD_FIELDS: &[&str] = &[
    "$schema",
    "schemaVersion",
    "enabled",
    "targets",
    "launcher",
    "links",
    "changelog",
    "settings",
    "groups",
    "presets",
];

fn unknown_manifest_fields_allowed(root: &Path) -> Result<bool, String> {
    let config = shroudforge_loader_config::config::read_loader(root)?;
    let detection = &config["modules"]["modDetection"];
    Ok(detection["enabled"].as_bool().unwrap_or(true)
        && detection["tolerateUnknownManifestFields"]
            .as_bool()
            .unwrap_or(true))
}

fn validate_mod_document(root: &Path, value: &Value, allow_unknown: bool) -> Result<Value, String> {
    validate_known_fields(
        root,
        crate::documents::MOD,
        value,
        MOD_FIELDS,
        allow_unknown,
    )
}

pub(crate) fn validate_extended_mod_document(root: &Path, value: &Value) -> Result<Value, String> {
    let allow_unknown = unknown_manifest_fields_allowed(root)?;
    validate_extended_mod_document_with_policy(root, value, allow_unknown)
}

fn validate_extended_mod_document_with_policy(
    root: &Path,
    value: &Value,
    allow_unknown: bool,
) -> Result<Value, String> {
    validate_known_fields(
        root,
        crate::documents::EXTENDED_MOD,
        value,
        EXTENDED_MOD_FIELDS,
        allow_unknown,
    )
}

fn validate_known_fields(
    root: &Path,
    document: shroudforge_loader_config::Document,
    value: &Value,
    known_fields: &[&str],
    allow_unknown: bool,
) -> Result<Value, String> {
    let mut normalized = value.clone();
    let object = normalized
        .as_object_mut()
        .ok_or_else(|| format!("{} must be an object", document.name))?;
    let unknown: Vec<_> = object
        .keys()
        .filter(|key| !known_fields.contains(&key.as_str()))
        .cloned()
        .collect();
    if !allow_unknown && !unknown.is_empty() {
        return Err(format!(
            "{}: unknown field{}: {}. Enable Mod Detection to ignore unknown top-level fields.",
            document.name,
            if unknown.len() == 1 { "" } else { "s" },
            unknown.join(", ")
        ));
    }
    for key in unknown {
        object.remove(&key);
    }
    crate::config::validate_document(root, document, &normalized)?;
    Ok(normalized)
}

pub fn parse_manifest(root: &Path, value: Value) -> Result<ModManifest, String> {
    parse_manifest_with_extension(root, value, None)
}

pub fn parse_manifest_with_extension(
    root: &Path,
    mut value: Value,
    external: Option<Value>,
) -> Result<ModManifest, String> {
    let had_extension = external.is_some();
    let allow_unknown = unknown_manifest_fields_allowed(root)?;
    let extension = external
        .map(|extension| {
            validate_extended_mod_document_with_policy(root, &extension, allow_unknown)
        })
        .transpose()?;
    let mut extension = extension;
    if let Some(extension) = extension.as_mut() {
        if let Some(object) = extension.as_object_mut() {
            object.remove("$schema");
        }
    }
    value = validate_mod_document(root, &value, allow_unknown)?;
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
        None if had_extension => crate::ModTarget::Client,
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
        definitions.push(definition);
    }
    validate_settings(&definitions, &Value::Object(effective.clone()))?;
    let explicit_schedule_keys: BTreeSet<_> = definitions
        .iter()
        .filter(|definition| definition.apply_at_explicit)
        .map(|definition| definition.key.clone())
        .collect();
    let groups = extension
        .get("groups")
        .map(|value| {
            serde_json::from_value::<Vec<SettingGroup>>(value.clone())
                .map_err(|error| format!("extended.mod.json groups: {error}"))
        })
        .transpose()?
        .unwrap_or_default();
    let presets = extension
        .get("presets")
        .map(|value| {
            serde_json::from_value::<BTreeMap<String, SettingPreset>>(value.clone())
                .map_err(|error| format!("extended.mod.json presets: {error}"))
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
    object.insert("presets".into(), json!(presets));
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
    let mut manifest: ModManifest =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    for setting in &mut manifest.settings {
        setting.apply_at_explicit = explicit_schedule_keys.contains(&setting.key);
    }
    validate_manifest(&manifest)?;
    Ok(manifest)
}
