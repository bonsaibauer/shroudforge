use shroudforge_hotkeys::InputDevice;
use std::path::Path;

/// Saves the player's padlock of one keybind on one device (true: the game
/// is denied it while the mod's context is active) in the
/// `inputReservations.choices` of extended.mod.json.
pub fn save_key_reservation(
    root: &Path,
    id: &str,
    key: &str,
    device: InputDevice,
    reserved: bool,
    revision: &str,
) -> Result<(), String> {
    crate::update_mod_configuration::update_mod_configuration(root, id, revision, |extension| {
        let block = extension
            .get_mut("inputReservations")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("this mod declares no input reservations")?;
        if block.get("keys").and_then(|keys| keys.get(key)).is_none() {
            return Err(format!("setting '{key}' cannot be reserved"));
        }
        let choices = block
            .entry("choices")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or("inputReservations choices must be an object")?;
        let choice = choices.entry(key).or_insert_with(|| serde_json::json!({}));
        let device = match device {
            InputDevice::Keyboard => "keyboard",
            InputDevice::Mouse => "mouse",
        };
        choice[device] = reserved.into();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn saves_a_padlock_among_the_choices() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("shroudforge-key-reservation-{suffix}"));
        let package = shroudforge_loader_config::paths::mods_dir(&root).join("example");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("mod.json"),
            br#"{"id":"example","name":"Example","version":"1.0.0"}"#,
        )
        .unwrap();
        std::fs::write(
            package.join("extended.mod.json"),
            br#"{"schemaVersion":1,"enabled":true,"targets":["client"],
                "settings":{"keyMark":{"value":"SDL:30","control":"keybind"}},
                "inputReservations":{"contexts":{"hammer":"Hammer in hand"},
                    "keys":{"keyMark":{"when":"hammer","keyboard":true}}}}"#,
        )
        .unwrap();
        let revision = crate::config_revision(
            &serde_json::json!({"enabled":true,"settings":{"keyMark":"SDL:30"}}),
        );

        save_key_reservation(
            &root,
            "example",
            "keyMark",
            InputDevice::Keyboard,
            false,
            &revision,
        )
        .unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(package.join("extended.mod.json")).unwrap())
                .unwrap();
        assert_eq!(
            saved["inputReservations"]["choices"]["keyMark"]["keyboard"],
            false
        );
        assert_eq!(saved["settings"]["keyMark"]["value"], "SDL:30");
        let refused = save_key_reservation(
            &root,
            "example",
            "keyOther",
            InputDevice::Mouse,
            true,
            &revision,
        );
        assert!(refused.unwrap_err().contains("cannot be reserved"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
