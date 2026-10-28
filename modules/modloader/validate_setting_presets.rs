use crate::{ModManifest, SettingControl, SettingValueType};
use std::collections::HashMap;

/// Check that each preset controller and member is a declared setting, that a
/// setting belongs to one preset at most, and that every preset gives a valid
/// value to each of its members.
pub(crate) fn validate_setting_presets(manifest: &ModManifest) -> Result<(), String> {
    let mut owners: HashMap<&str, &str> = HashMap::new();
    for (controller_key, preset) in &manifest.presets {
        let controller = manifest
            .settings
            .iter()
            .find(|setting| setting.key == *controller_key)
            .ok_or_else(|| format!("preset '{controller_key}' needs a setting with that key"))?;
        if controller.value_type != SettingValueType::String {
            return Err(format!(
                "preset '{controller_key}' needs a string setting as its controller"
            ));
        }
        let controller_options: Vec<&str> = controller
            .options
            .iter()
            .filter_map(|option| option.value.as_str())
            .collect();
        if !controller_options.is_empty() && !controller_options.contains(&preset.custom.as_str()) {
            return Err(format!(
                "preset '{controller_key}' custom value '{}' is not an option of its setting",
                preset.custom
            ));
        }
        if preset.settings.is_empty() {
            return Err(format!("preset '{controller_key}' has no settings"));
        }
        for key in &preset.settings {
            if key == controller_key {
                return Err(format!(
                    "preset '{controller_key}' cannot list its own setting"
                ));
            }
            let member = manifest
                .settings
                .iter()
                .find(|setting| setting.key == *key)
                .ok_or_else(|| {
                    format!("preset '{controller_key}' references unknown setting '{key}'")
                })?;
            if matches!(member.control, Some(SettingControl::Keybind)) {
                return Err(format!(
                    "preset '{controller_key}' cannot set keybind setting '{key}'"
                ));
            }
            if manifest.presets.contains_key(key) {
                return Err(format!(
                    "preset '{controller_key}' cannot set '{key}', which controls another preset"
                ));
            }
            if let Some(owner) = owners.insert(key.as_str(), controller_key.as_str()) {
                return Err(if owner == controller_key {
                    format!("preset '{controller_key}' lists setting '{key}' twice")
                } else {
                    format!("setting '{key}' belongs to presets '{owner}' and '{controller_key}'")
                });
            }
        }
        if preset.values.is_empty() {
            return Err(format!("preset '{controller_key}' has no values"));
        }
        for option in &controller_options {
            if *option != preset.custom && !preset.values.contains_key(*option) {
                return Err(format!(
                    "preset '{controller_key}' has no values for option '{option}'"
                ));
            }
        }
        for (name, values) in &preset.values {
            if *name == preset.custom {
                return Err(format!(
                    "preset '{controller_key}' cannot give values to its custom value '{name}'"
                ));
            }
            if !controller_options.is_empty() && !controller_options.contains(&name.as_str()) {
                return Err(format!(
                    "preset '{controller_key}' value '{name}' is not an option of its setting"
                ));
            }
            for key in &preset.settings {
                if !values.contains_key(key) {
                    return Err(format!(
                        "preset '{controller_key}' value '{name}' does not set '{key}'"
                    ));
                }
            }
            if let Some(extra) = values.keys().find(|key| !preset.settings.contains(key)) {
                return Err(format!(
                    "preset '{controller_key}' value '{name}' sets '{extra}', which is not in its settings"
                ));
            }
            crate::config::validate_settings(
                &manifest.settings,
                &serde_json::Value::Object(values.clone()),
            )
            .map_err(|error| format!("preset '{controller_key}' value '{name}': {error}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::parse_manifest_with_extension;
    use serde_json::{Value, json};
    use std::path::Path;

    fn extension(presets: Value) -> Value {
        json!({
            "schemaVersion": 1,
            "enabled": true,
            "settings": {
                "quality": {
                    "value": "medium",
                    "control": "select",
                    "options": {"low": "Low", "medium": "Medium", "custom": "Custom"}
                },
                "range": {"value": 128, "min": 32, "max": 512},
                "detail": {"value": "medium", "options": {"low": "Low", "medium": "Medium"}},
                "shadows": {"value": true},
                "toggleKey": {"value": "SDL:66", "control": "keybind"}
            },
            "presets": presets
        })
    }

    fn parse(presets: Value) -> Result<crate::ModManifest, String> {
        parse_manifest_with_extension(
            Path::new("."),
            json!({
                "id": "preset-example",
                "name": "Preset Example",
                "version": "1.0.0",
                "capabilities": ["runtime"]
            }),
            Some(extension(presets)),
        )
    }

    fn quality(values: Value) -> Value {
        json!({"quality": {"settings": ["range", "detail"], "values": values}})
    }

    fn rejects(presets: Value, message: &str) {
        let error = parse(presets).expect_err("the preset is refused");
        assert!(error.contains(message), "{error}");
    }

    #[test]
    fn presets_are_read_with_their_settings_and_values() {
        let manifest = parse(quality(json!({
            "low": {"range": 64, "detail": "low"},
            "medium": {"range": 128, "detail": "medium"}
        })))
        .expect("a valid preset is accepted");

        let preset = &manifest.presets["quality"];
        assert_eq!(preset.custom, "custom");
        assert_eq!(preset.settings, vec!["range", "detail"]);
        assert_eq!(preset.values["low"]["range"], 64);
        assert_eq!(
            serde_json::to_value(&manifest).unwrap()["presets"]["quality"]["values"]["medium"]["detail"],
            "medium"
        );
    }

    #[test]
    fn manifests_without_presets_serialize_as_before() {
        let manifest = parse(json!({})).expect("an empty preset list is accepted");
        assert!(
            serde_json::to_value(&manifest)
                .unwrap()
                .get("presets")
                .is_none()
        );
    }

    #[test]
    fn preset_controller_and_members_must_be_declared_settings() {
        rejects(
            json!({"speed": {"settings": ["range"], "values": {"fast": {"range": 64}}}}),
            "preset 'speed' needs a setting with that key",
        );
        rejects(
            json!({"shadows": {"settings": ["range"], "values": {"on": {"range": 64}}}}),
            "preset 'shadows' needs a string setting as its controller",
        );
        rejects(
            json!({"quality": {"settings": ["range", "missing"], "values": {"low": {"range": 64}}}}),
            "preset 'quality' references unknown setting 'missing'",
        );
        rejects(
            json!({"quality": {"settings": ["quality"], "values": {"low": {"range": 64}}}}),
            "preset 'quality' cannot list its own setting",
        );
        rejects(
            json!({"quality": {"settings": ["toggleKey"], "values": {"low": {"range": 64}}}}),
            "preset 'quality' cannot set keybind setting 'toggleKey'",
        );
    }

    #[test]
    fn a_setting_belongs_to_one_preset_at_most() {
        rejects(
            json!({
                "quality": {"settings": ["range"], "values": {"low": {"range": 64}, "medium": {"range": 64}}},
                "detail": {"custom": "medium", "settings": ["range"], "values": {"low": {"range": 64}}}
            }),
            "setting 'range' belongs to presets 'detail' and 'quality'",
        );
        rejects(
            json!({
                "quality": {"settings": ["detail"], "values": {"low": {"detail": "low"}, "medium": {"detail": "low"}}},
                "detail": {"custom": "medium", "settings": ["range"], "values": {"low": {"range": 64}}}
            }),
            "preset 'quality' cannot set 'detail', which controls another preset",
        );
    }

    #[test]
    fn preset_values_match_the_controller_options() {
        rejects(
            quality(json!({"low": {"range": 64, "detail": "low"}})),
            "preset 'quality' has no values for option 'medium'",
        );
        rejects(
            quality(json!({
                "low": {"range": 64, "detail": "low"},
                "medium": {"range": 64, "detail": "low"},
                "ultra": {"range": 64, "detail": "low"}
            })),
            "preset 'quality' value 'ultra' is not an option of its setting",
        );
        rejects(
            quality(json!({
                "low": {"range": 64, "detail": "low"},
                "medium": {"range": 64, "detail": "low"},
                "custom": {"range": 64, "detail": "low"}
            })),
            "preset 'quality' cannot give values to its custom value 'custom'",
        );
        rejects(
            json!({"quality": {
                "custom": "manual",
                "settings": ["range"],
                "values": {"low": {"range": 64}, "medium": {"range": 64}}
            }}),
            "preset 'quality' custom value 'manual' is not an option of its setting",
        );
    }

    #[test]
    fn preset_values_set_each_member_with_a_valid_value() {
        rejects(
            quality(json!({
                "low": {"range": 64},
                "medium": {"range": 128, "detail": "medium"}
            })),
            "preset 'quality' value 'low' does not set 'detail'",
        );
        rejects(
            quality(json!({
                "low": {"range": 64, "detail": "low", "shadows": false},
                "medium": {"range": 128, "detail": "medium"}
            })),
            "preset 'quality' value 'low' sets 'shadows', which is not in its settings",
        );
        rejects(
            quality(json!({
                "low": {"range": 8, "detail": "low"},
                "medium": {"range": 128, "detail": "medium"}
            })),
            "preset 'quality' value 'low': range:",
        );
        rejects(
            quality(json!({
                "low": {"range": 64, "detail": "lowest"},
                "medium": {"range": 128, "detail": "medium"}
            })),
            "preset 'quality' value 'low': detail:",
        );
        rejects(
            quality(json!({
                "low": {"range": "far", "detail": "low"},
                "medium": {"range": 128, "detail": "medium"}
            })),
            "preset 'quality' value 'low': range:",
        );
    }
}
