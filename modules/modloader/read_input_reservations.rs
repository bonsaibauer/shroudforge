use crate::{KeyReservation, ReservedDevices, SettingDefinition, WheelReservation};
use serde::Deserialize;
use std::collections::BTreeMap;

/// The `inputReservations` block of extended.mod.json. It sits at the top
/// level, where a ShroudForge without input reservations ignores it, so the
/// same mod still loads there.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InputReservations {
    #[serde(default)]
    contexts: BTreeMap<String, String>,
    #[serde(default)]
    keys: BTreeMap<String, KeyReservation>,
    #[serde(default)]
    wheels: BTreeMap<String, WheelReservation>,
    /// The player's padlocks, written by the Modloader.
    #[serde(default)]
    choices: BTreeMap<String, ReservedDevices>,
}

/// Puts the reservations of `block` on the settings they name and returns the
/// mod's input contexts (validate_input_reservations checks them).
pub(crate) fn read_input_reservations(
    block: Option<&serde_json::Value>,
    definitions: &mut [SettingDefinition],
) -> Result<BTreeMap<String, String>, String> {
    let Some(block) = block else {
        return Ok(BTreeMap::new());
    };
    let reservations: InputReservations = serde_json::from_value(block.clone())
        .map_err(|error| format!("extended.mod.json inputReservations: {error}"))?;
    let mut setting = |key: &str, part: &str| {
        definitions
            .iter_mut()
            .position(|definition| definition.key == key)
            .ok_or_else(|| format!("inputReservations {part} names unknown setting '{key}'"))
    };
    let mut placed = Vec::new();
    for (key, reserve) in reservations.keys {
        placed.push((setting(&key, "keys")?, Some(reserve), None, None));
    }
    for (key, wheel) in reservations.wheels {
        placed.push((setting(&key, "wheels")?, None, Some(wheel), None));
    }
    for (key, choice) in reservations.choices {
        placed.push((setting(&key, "choices")?, None, None, Some(choice)));
    }
    for (index, reserve, wheel, choice) in placed {
        let definition = &mut definitions[index];
        if reserve.is_some() {
            definition.reserve = reserve;
        }
        if wheel.is_some() {
            definition.wheel = wheel;
        }
        if choice.is_some() {
            definition.reserved = choice;
        }
    }
    Ok(reservations.contexts)
}
