//! Compare declared client shortcuts without loading or executing mod packages.
use crate::{SettingControl, read_manifest_path};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shroudforge_hotkeys::Hotkey;
use std::{fs, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedHotkey {
    pub scancode: u16,
    pub label: String,
    pub action: ProtectedHotkeyAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProtectedHotkeyAction {
    Block,
    Confirm,
    Cancel,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyConflict {
    pub first_id: String,
    pub second_id: String,
    pub binding: String,
}

pub enum HotkeyAssessment {
    Allowed,
    Confirm(ProtectedHotkey),
    Cancel,
}

pub enum HotkeyAssessmentTarget {
    Module {
        module: String,
        binding: Value,
    },
    Mod {
        id: String,
        key: String,
        values: Value,
    },
}

struct ConfiguredBinding {
    id: String,
    binding: Hotkey,
    changed: bool,
}

pub fn protected_hotkeys(config: &Value) -> Vec<ProtectedHotkey> {
    config
        .pointer("/hotkeys/protectedKeys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| serde_json::from_value(value.clone()).ok())
        .collect()
}

pub fn find_hotkey_conflicts(bindings: &[(String, Hotkey)]) -> Vec<HotkeyConflict> {
    let mut conflicts = Vec::new();
    for (index, (first_id, first)) in bindings.iter().enumerate() {
        for (second_id, second) in &bindings[index + 1..] {
            if first.overlaps(second) {
                conflicts.push(HotkeyConflict {
                    first_id: first_id.clone(),
                    second_id: second_id.clone(),
                    binding: first.normalized(),
                });
            }
        }
    }
    conflicts
}

fn reject_protected_binding(
    binding: &Hotkey,
    protected: &[ProtectedHotkey],
) -> Result<(), String> {
    for scancode in binding.keyboard_scancodes() {
        if let Some(key) = protected.iter().find(|key| key.scancode == scancode) {
            if matches!(
                key.action,
                ProtectedHotkeyAction::Block | ProtectedHotkeyAction::Cancel
            ) {
                return Err(format!(
                    "{} is protected by ShroudForge's hotkey configuration",
                    key.label
                ));
            }
        }
    }
    Ok(())
}

/// Check the proposed loader configuration and optional partial mod settings.
/// Definitions and values remain in their existing configuration documents.
pub fn validate_hotkey_configuration(
    root: &Path,
    config: &Value,
    changed_mod: Option<(&str, &Value, Option<bool>)>,
) -> Result<(), String> {
    let previous_config = crate::config::read_loader(root)?;
    let mut bindings = Vec::new();
    let protected = protected_hotkeys(config);
    if let Some(modules) = config.get("modules").and_then(Value::as_object) {
        for (id, module) in modules {
            let Some(value) = module.get("toggleKey") else {
                continue;
            };
            let binding = shroudforge_hotkeys::parse_value(value)?;
            let previous_module = previous_config
                .get("modules")
                .and_then(|modules| modules.get(id));
            let previous_binding = previous_module
                .and_then(|module| module.get("toggleKey"))
                .and_then(|value| shroudforge_hotkeys::parse_value(value).ok().flatten());
            if previous_binding.as_ref() != binding.as_ref() {
                if let Some(binding) = &binding {
                    reject_protected_binding(binding, &protected)?;
                }
            }
            let previously_enabled = previous_module
                .and_then(|module| module.get("enabled"))
                .and_then(Value::as_bool)
                != Some(false);
            let changed = !previously_enabled || previous_binding.as_ref() != binding.as_ref();
            if module.get("enabled").and_then(Value::as_bool) == Some(false) {
                continue;
            }
            if let Some(binding) = binding {
                bindings.push(ConfiguredBinding {
                    id: format!("modules.{id}.toggleKey"),
                    changed,
                    binding,
                });
            }
        }
    }

    let packages = match fs::read_dir(crate::paths::mods_dir(root)) {
        Ok(packages) => Some(packages),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Could not inspect mod hotkeys: {error}")),
    };
    for entry in packages.into_iter().flatten() {
        let entry = entry.map_err(|error| format!("Could not inspect mod hotkeys: {error}"))?;
        let package = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.')
            || !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            || !package.join("mod.json").is_file()
        {
            continue;
        }
        let manifest = match read_manifest_path(root, &package) {
            Ok(manifest) => manifest,
            Err(error) => {
                tracing::warn!(target: "shroudforge::modloader", path = %package.display(),
                    "Could not inspect declared mod hotkeys: {error}");
                continue;
            }
        };
        if let Some((_, overrides, _)) = changed_mod.filter(|(id, _, _)| manifest.id == *id) {
            let overrides = overrides
                .as_object()
                .ok_or("mod settings must be an object")?;
            for setting in &manifest.settings {
                if !matches!(setting.control, Some(SettingControl::Keybind)) {
                    continue;
                }
                let Some(value) = overrides.get(&setting.key) else {
                    continue;
                };
                let binding = shroudforge_hotkeys::parse_value(value)?;
                let previous_binding = manifest
                    .setting_values
                    .get(&setting.key)
                    .and_then(|value| shroudforge_hotkeys::parse_value(value).ok().flatten());
                if previous_binding.as_ref() != binding.as_ref() {
                    if let Some(binding) = &binding {
                        reject_protected_binding(binding, &protected)?;
                    }
                }
            }
        }
        let enabled = changed_mod
            .filter(|(id, _, _)| manifest.id == *id)
            .and_then(|(_, _, enabled)| enabled)
            .unwrap_or(manifest.enabled);
        if !enabled || !manifest.target.supports_process(false) {
            continue;
        }
        if manifest.id == "world-editor"
            && config
                .pointer("/modules/worldEditor/enabled")
                .and_then(Value::as_bool)
                == Some(false)
        {
            continue;
        }
        let previously_active = manifest.enabled
            && (manifest.id != "world-editor"
                || previous_config
                    .pointer("/modules/worldEditor/enabled")
                    .and_then(Value::as_bool)
                    != Some(false));
        let mut values = manifest.setting_values.clone();
        if let Some((id, overrides, _)) = changed_mod {
            if manifest.id == id {
                let overrides = overrides
                    .as_object()
                    .ok_or("mod settings must be an object")?;
                let settings = values
                    .as_object_mut()
                    .ok_or("mod settings must be an object")?;
                for (key, value) in overrides {
                    settings.insert(key.clone(), value.clone());
                }
            }
        }
        for setting in &manifest.settings {
            if !matches!(setting.control, Some(SettingControl::Keybind)) {
                continue;
            }
            if !crate::setting_is_enabled(&setting.key, &manifest.settings, &values) {
                continue;
            }
            let value = values.get(&setting.key).ok_or_else(|| {
                format!("Missing hotkey setting '{}.{}'", manifest.id, setting.key)
            })?;
            if let Some(binding) = shroudforge_hotkeys::parse_value(value)? {
                let previous_binding = if previously_active
                    && crate::setting_is_enabled(
                        &setting.key,
                        &manifest.settings,
                        &manifest.setting_values,
                    ) {
                    manifest
                        .setting_values
                        .get(&setting.key)
                        .and_then(|value| shroudforge_hotkeys::parse_value(value).ok().flatten())
                } else {
                    None
                };
                bindings.push(ConfiguredBinding {
                    id: format!("{}.{}", manifest.id, setting.key),
                    changed: previous_binding.as_ref() != Some(&binding),
                    binding,
                });
            }
        }
    }
    bindings.sort_by(|left, right| left.id.cmp(&right.id));
    for current in bindings.iter().filter(|binding| binding.changed) {
        reject_protected_binding(&current.binding, &protected)?;
    }
    let changed_ids: Vec<_> = bindings
        .iter()
        .filter(|binding| binding.changed)
        .map(|binding| binding.id.as_str())
        .collect();
    for conflict in find_hotkey_conflicts(
        &bindings
            .iter()
            .map(|binding| (binding.id.clone(), binding.binding.clone()))
            .collect::<Vec<_>>(),
    ) {
        if changed_ids.contains(&conflict.first_id.as_str())
            || changed_ids.contains(&conflict.second_id.as_str())
        {
            return Err(format!(
                "Hotkey {} conflicts between '{}' and '{}'. Choose another binding or clear one shortcut",
                conflict.binding, conflict.first_id, conflict.second_id,
            ));
        }
    }
    Ok(())
}

/// Check a proposed keybind before the UI asks for confirmation or saves it.
/// The save path runs `validate_hotkey_configuration` again before writing.
pub fn assess_hotkey_change(
    root: &Path,
    target: &HotkeyAssessmentTarget,
) -> Result<HotkeyAssessment, String> {
    match target {
        HotkeyAssessmentTarget::Module { module, binding } => {
            let mut config = crate::config::read_loader(root)?;
            let module_settings = config
                .get_mut("modules")
                .and_then(Value::as_object_mut)
                .and_then(|modules| modules.get_mut(module))
                .and_then(Value::as_object_mut)
                .ok_or("module settings were not found")?;
            module_settings.insert("toggleKey".into(), binding.clone());
            assess_binding(root, &config, binding, None)
        }
        HotkeyAssessmentTarget::Mod { id, key, values } => {
            let package = crate::package_lookup::find_mod_package(root, id)
                .ok_or("mod package not found")?;
            let manifest = crate::config::read_manifest_path(root, &package)?;
            if !manifest.settings.iter().any(|setting| {
                setting.key == *key
                    && matches!(setting.control, Some(SettingControl::Keybind))
            }) {
                return Err(format!("'{key}' is not a declared keybind setting"));
            }
            let binding = values
                .get(key)
                .ok_or_else(|| format!("Missing hotkey setting '{}.{key}'", id))?;
            let config = crate::config::read_loader(root)?;
            assess_binding(root, &config, binding, Some((id, values, None)))
        }
    }
}

fn assess_binding(
    root: &Path,
    config: &Value,
    value: &Value,
    changed_mod: Option<(&str, &Value, Option<bool>)>,
) -> Result<HotkeyAssessment, String> {
    let binding = shroudforge_hotkeys::parse_value(value)?;
    let policy = binding.as_ref().and_then(|binding| {
        let protected = protected_hotkeys(config);
        binding
            .keyboard_scancodes()
            .find_map(|scancode| {
                protected
                    .iter()
                    .find(|key| key.scancode == scancode)
                    .cloned()
            })
    });
    if let Some(key) = &policy {
        match key.action {
            ProtectedHotkeyAction::Cancel => return Ok(HotkeyAssessment::Cancel),
            ProtectedHotkeyAction::Block => {
                return Err(format!(
                    "{} is protected by ShroudForge's hotkey configuration",
                    key.label
                ));
            }
            ProtectedHotkeyAction::Confirm => {}
        }
    }
    validate_hotkey_configuration(root, config, changed_mod)?;
    Ok(match policy {
        Some(key) => HotkeyAssessment::Confirm(key),
        None => HotkeyAssessment::Allowed,
    })
}
