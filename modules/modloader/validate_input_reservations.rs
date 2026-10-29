use crate::package_lookup::valid_identifier;
use crate::{ModManifest, SettingControl, SettingValueType};

/// Checks the input contexts of a mod and the settings that name them: a
/// keybind may be reserved, a boolean may read the mouse wheel, each while a
/// declared context is active.
pub(crate) fn validate_input_reservations(manifest: &ModManifest) -> Result<(), String> {
    for (context, label) in &manifest.input_contexts {
        if !valid_identifier(context) {
            return Err(format!("input context '{context}' is not a valid identifier"));
        }
        if label.trim().is_empty() || label.chars().count() > 120 {
            return Err(format!(
                "input context '{context}' needs a label of 1 to 120 characters"
            ));
        }
    }
    let known = |when: &str| manifest.input_contexts.contains_key(when);
    for setting in &manifest.settings {
        if let Some(reserve) = &setting.reserve {
            if !matches!(setting.control, Some(SettingControl::Keybind)) {
                return Err(format!(
                    "setting '{}' reserve is only valid on a keybind",
                    setting.key
                ));
            }
            if !known(&reserve.when) {
                return Err(format!(
                    "setting '{}' reserve names unknown input context '{}'",
                    setting.key, reserve.when
                ));
            }
        } else if setting.reserved.is_some() {
            return Err(format!(
                "setting '{}' has reserved devices but no reserve",
                setting.key
            ));
        }
        if let Some(wheel) = &setting.wheel {
            if setting.value_type != SettingValueType::Boolean || setting.reserve.is_some() {
                return Err(format!(
                    "setting '{}' wheel is only valid on a boolean setting",
                    setting.key
                ));
            }
            if !known(&wheel.when) {
                return Err(format!(
                    "setting '{}' wheel names unknown input context '{}'",
                    setting.key, wheel.when
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(extension: serde_json::Value) -> Result<ModManifest, String> {
        crate::parse_manifest_with_extension(
            std::path::Path::new("."),
            serde_json::json!({"id":"example","name":"Example","version":"1.0.0"}),
            Some(extension),
        )
    }

    #[test]
    fn reads_reservations_and_the_wheel() {
        let parsed = manifest(serde_json::json!({
            "schemaVersion": 1, "enabled": true,
            "settings": {
                "keyMark": {"value": "SDL:30", "control": "keybind"},
                "keyUndo": {"value": "SDL:64", "control": "keybind"},
                "wheelDistance": {"value": true, "control": "toggle"}
            },
            "inputReservations": {
                "contexts": {"hammer": "Hammer in hand"},
                "keys": {
                    "keyMark": {"when": "hammer", "keyboard": true},
                    "keyUndo": {"when": "hammer"}
                },
                "wheels": {"wheelDistance": {"when": "hammer"}},
                "choices": {"keyMark": {"keyboard": false}}
            }
        }))
        .unwrap();
        let setting = |key: &str| parsed.settings.iter().find(|s| s.key == key).unwrap();
        use shroudforge_hotkeys::InputDevice::{Keyboard, Mouse};
        assert!(!setting("keyMark").reserves(Keyboard), "the player's padlock wins");
        assert!(!setting("keyUndo").reserves(Keyboard) && !setting("keyUndo").reserves(Mouse));
        assert_eq!(setting("wheelDistance").wheel.as_ref().unwrap().when, "hammer");
        assert_eq!(parsed.input_contexts["hammer"], "Hammer in hand");
    }

    #[test]
    fn rejects_unknown_contexts_and_misplaced_reservations() {
        let with = |settings: serde_json::Value, reservations: serde_json::Value| {
            manifest(serde_json::json!({
                "schemaVersion": 1, "enabled": true, "settings": settings, "inputReservations": reservations
            }))
            .unwrap_err()
        };
        let keybind = serde_json::json!({"keyMark": {"value": "SDL:30", "control": "keybind"}});
        assert!(with(keybind.clone(), serde_json::json!({"keys": {"keyMark": {"when": "hammer"}}}))
            .contains("unknown input context"));
        assert!(with(keybind.clone(), serde_json::json!({"keys": {"other": {"when": "hammer"}}}))
            .contains("unknown setting 'other'"));
        assert!(with(
            serde_json::json!({"size": {"value": 2}}),
            serde_json::json!({"contexts": {"hammer": "Hammer in hand"},
                "keys": {"size": {"when": "hammer"}}})
        )
        .contains("only valid on a keybind"));
        assert!(with(
            serde_json::json!({"name": {"value": "x"}}),
            serde_json::json!({"contexts": {"hammer": "Hammer in hand"},
                "wheels": {"name": {"when": "hammer"}}})
        )
        .contains("only valid on a boolean"));
        assert!(with(keybind, serde_json::json!({"choices": {"keyMark": {"keyboard": true}}}))
            .contains("no reserve"));
    }
}
