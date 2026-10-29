//! A mod's input reservations (extended.mod.json `inputReservations`) as the
//! game input filter of shroudforge-hotkeys needs them, and the Lua functions
//! `shroudforge.input.set_context` and `shroudforge.input.wheel_steps`.
//! Kept per mod so that a binding or a padlock changed in the Modloader
//! applies at the mod's next report, without restarting the game.

use mlua::{Table, Value};
use mod_loader::{SettingControl, SettingDefinition};
use shroudforge_hotkeys::{InputDevice, ReadWheel, ReservedKey};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
};

struct ModReservations {
    /// All settings of the mod (enabledWhen may name any of them).
    settings: Vec<SettingDefinition>,
    contexts: Vec<String>,
    /// What the filter has, to send it again only after a change.
    applied: Option<String>,
}

static MOD_RESERVATIONS: Mutex<Option<HashMap<String, ModReservations>>> = Mutex::new(None);

/// Records the reservations of `mod_id` from its settings: at its start, and
/// again when the player changed a padlock.
pub(crate) fn register_input_reservations(
    mod_id: &str,
    settings: &[SettingDefinition],
    contexts: &BTreeMap<String, String>,
) {
    let Ok(mut guard) = MOD_RESERVATIONS.lock() else {
        return;
    };
    guard.get_or_insert_with(HashMap::new).insert(
        mod_id.to_owned(),
        ModReservations {
            settings: settings.to_vec(),
            contexts: contexts.keys().cloned().collect(),
            applied: None,
        },
    );
}

/// Sends the reservations of `mod_id`, with its keybinds as bound now, to
/// the input filter when they changed since the last time.
fn send_reservations(lua: &mlua::Lua, mod_id: &str) -> mlua::Result<()> {
    let Ok(mut guard) = MOD_RESERVATIONS.lock() else {
        return Ok(());
    };
    let Some(reservations) = guard.as_mut().and_then(|all| all.get_mut(mod_id)) else {
        return Ok(());
    };
    let current: Table = lua.named_registry_value(&format!("shroudforge.settings.{mod_id}"))?;
    let mut keys = Vec::new();
    let mut wheels = Vec::new();
    let mut signature = String::new();
    for setting in &reservations.settings {
        if let Some(reserve) = &setting.reserve
            && matches!(setting.control, Some(SettingControl::Keybind))
        {
            let binding = super::create_mod_api::read_keybind(
                lua,
                mod_id,
                &reservations.settings,
                &setting.key,
            )?;
            for device in [InputDevice::Keyboard, InputDevice::Mouse] {
                if !setting.reserves(device) {
                    continue;
                }
                if let Some(hotkey) = binding.as_ref().and_then(|hotkey| hotkey.on_device(device)) {
                    signature.push_str(&format!("{}={};", setting.key, hotkey.normalized()));
                    keys.push(ReservedKey {
                        context: reserve.when.clone(),
                        hotkey,
                    });
                }
            }
        }
        if let Some(wheel) = &setting.wheel {
            let reserved = matches!(current.raw_get::<Value>(setting.key.as_str())?, Value::Boolean(true));
            signature.push_str(&format!("{}={reserved};", setting.key));
            wheels.push(ReadWheel {
                setting: setting.key.clone(),
                context: wheel.when.clone(),
                reserved,
            });
        }
    }
    if reservations.applied.as_deref() != Some(signature.as_str()) {
        shroudforge_hotkeys::set_reserved_input(mod_id, keys, wheels);
        reservations.applied = Some(signature);
    }
    Ok(())
}

fn declared(mod_id: &str, check: impl Fn(&ModReservations) -> bool) -> bool {
    MOD_RESERVATIONS
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref()?.get(mod_id).map(&check))
        .unwrap_or(false)
}

/// Adds `set_context` and `wheel_steps` to the mod's `shroudforge.input`.
pub(super) fn add_input_reservation_api(
    lua: &mlua::Lua,
    input: &Table,
    mod_id: &str,
) -> mlua::Result<()> {
    let context_mod = mod_id.to_owned();
    input.raw_set(
        "set_context",
        lua.create_function(move |lua, (context, active): (String, bool)| {
            if !declared(&context_mod, |mod_reservations| {
                mod_reservations.contexts.contains(&context)
            }) {
                return Err(mlua::Error::runtime(format!(
                    "input context '{context}' is not declared in extended.mod.json inputReservations.contexts"
                )));
            }
            send_reservations(lua, &context_mod)?;
            if active {
                // Once per game, and again when the game replaced its window;
                // nothing to filter outside the game client.
                let _ = shroudforge_hotkeys::install_input_filter();
            }
            shroudforge_hotkeys::set_input_context(&context_mod, &context, active);
            Ok(())
        })?,
    )?;
    let wheel_mod = mod_id.to_owned();
    input.raw_set(
        "wheel_steps",
        lua.create_function(move |_, setting: String| {
            if !declared(&wheel_mod, |mod_reservations| {
                mod_reservations
                    .settings
                    .iter()
                    .any(|candidate| candidate.key == setting && candidate.wheel.is_some())
            }) {
                return Err(mlua::Error::runtime(format!(
                    "setting '{setting}' is not declared in extended.mod.json inputReservations.wheels"
                )));
            }
            Ok(shroudforge_hotkeys::take_wheel_steps(&wheel_mod, &setting))
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(key: &str, value: serde_json::Value, extra: serde_json::Value) -> SettingDefinition {
        let mut definition = serde_json::json!({
            "key": key, "type": if value.is_boolean() { "boolean" } else { "string" },
            "label": key, "default": value, "applyAt": "live"
        });
        for (name, field) in extra.as_object().unwrap() {
            definition[name] = field.clone();
        }
        serde_json::from_value(definition).unwrap()
    }

    #[test]
    fn reports_contexts_and_reads_the_wheel_of_declared_settings() -> mlua::Result<()> {
        let lua = mlua::Lua::new();
        let settings = vec![
            setting(
                "keyMark",
                serde_json::json!("SDL:30"),
                serde_json::json!({"control": "keybind",
                    "reserve": {"when": "hammer", "keyboard": true}}),
            ),
            setting(
                "wheelDistance",
                serde_json::json!(true),
                serde_json::json!({"control": "toggle", "wheel": {"when": "free"}}),
            ),
        ];
        let contexts = BTreeMap::from([
            ("hammer".to_owned(), "Hammer in hand".to_owned()),
            ("free".to_owned(), "Free point on".to_owned()),
        ]);
        super::super::set_settings(
            &lua,
            "reserving-mod",
            &serde_json::json!({"keyMark": "SDL:30", "wheelDistance": true}),
        )?;
        register_input_reservations("reserving-mod", &settings, &contexts);
        let input = lua.create_table()?;
        add_input_reservation_api(&lua, &input, "reserving-mod")?;
        lua.globals().set("input", input)?;

        lua.load(r#"input.set_context("hammer", true); input.set_context("free", false)"#)
            .exec()?;
        assert_eq!(lua.load(r#"return input.wheel_steps("wheelDistance")"#).eval::<i32>()?, 0);
        let unknown = lua.load(r#"input.set_context("flying", true)"#).exec().unwrap_err();
        assert!(unknown.to_string().contains("input context 'flying' is not declared"));
        let not_a_wheel = lua.load(r#"return input.wheel_steps("keyMark")"#).exec().unwrap_err();
        assert!(not_a_wheel.to_string().contains("not declared in extended.mod.json inputReservations.wheels"));
        shroudforge_hotkeys::release_reserved_input("reserving-mod");
        Ok(())
    }
}
