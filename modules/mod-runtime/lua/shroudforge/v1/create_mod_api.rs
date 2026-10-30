use std::time::{SystemTime, UNIX_EPOCH};

use mlua::{Function, Table, Value, Variadic};
use mod_loader::Mod;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use super::read_ui_locale::read_ui_locale;
use super::set_mod_settings::set_settings;
use crate::lua::LuaValue;

pub fn create(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.raw_set("version", env!("CARGO_PKG_VERSION"))?;
    table.raw_set("contents", super::read_game_content::create(lua)?)?;
    table.raw_set("mod_id", r#mod.info().id.as_str())?;

    let log = lua.create_table()?;
    for level in ["trace", "debug", "info", "warn", "error"] {
        let id = r#mod.info().id.clone();
        log.raw_set(
            level,
            lua.create_function(move |_, args: Variadic<LuaValue>| {
                let disabled_detail_level = match level {
                    "trace" => {
                        !tracing::enabled!(target: "shroudforge::mod", tracing::Level::TRACE)
                    }
                    "debug" => {
                        !tracing::enabled!(target: "shroudforge::mod", tracing::Level::DEBUG)
                    }
                    _ => false,
                };
                if disabled_detail_level {
                    return Ok(());
                }
                let message = args
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect::<mlua::Result<Vec<_>>>()?
                    .join("\t");
                match level {
                    "trace" => {
                        tracing::trace!(target: "shroudforge::mod", mod_id = %id, "{message}")
                    }
                    "debug" => {
                        tracing::debug!(target: "shroudforge::mod", mod_id = %id, "{message}")
                    }
                    "info" => tracing::info!(target: "shroudforge::mod", mod_id = %id, "{message}"),
                    "warn" => tracing::warn!(target: "shroudforge::mod", mod_id = %id, "{message}"),
                    "error" => {
                        tracing::error!(target: "shroudforge::mod", mod_id = %id, "{message}")
                    }
                    _ => unreachable!(),
                }
                Ok(())
            })?,
        )?;
    }
    table.raw_set("log", log)?;

    let game_dir = lua
        .app_data_ref::<crate::env::AppState>()
        .unwrap()
        .env()
        .game_dir()
        .as_std_path()
        .to_path_buf();
    #[cfg(windows)]
    if lua
        .app_data_ref::<crate::env::AppState>()
        .is_some_and(|state| state.is_client() && state.phase() == crate::RuntimePhase::Ingame)
        && r#mod
            .info()
            .settings
            .iter()
            .any(|setting| matches!(setting.control, Some(mod_loader::SettingControl::Keybind)))
    {
        static INITIALIZE_INPUT: std::sync::Once = std::sync::Once::new();
        INITIALIZE_INPUT.call_once(|| {
            let executable = shroudforge_loader_config::paths::loader_executable(&game_dir);
            if let Err(error) = shroudforge_hotkeys::initialize_runtime(&executable) {
                tracing::warn!(target: "shroudforge::runtime", "Keyboard shortcuts unavailable: {error}");
            }
        });
    }
    let mod_id = r#mod.info().id.clone();

    let settings = lua.create_table()?;
    // In-memory values; the runtime refreshes explicitly live settings between updates.
    set_settings(lua, &mod_id, &r#mod.info().setting_values)?;
    let settings_id = mod_id.clone();
    settings.raw_set(
        "get",
        lua.create_function(move |lua, (key, fallback): (String, Option<Value>)| {
            if !valid_identifier(&key) {
                return Err(mlua::Error::runtime("invalid setting key"));
            }
            let current: Table =
                lua.named_registry_value(&format!("shroudforge.settings.{settings_id}"))?;
            let value: Value = current.raw_get(key)?;
            Ok(if value.is_nil() {
                fallback.unwrap_or(Value::Nil)
            } else {
                value
            })
        })?,
    )?;
    table.raw_set("settings", settings)?;

    let input = lua.create_table()?;
    let legacy_api_reported = Arc::new(AtomicBool::new(false));
    let legacy_mod_id = mod_id.clone();
    input.raw_set(
        "is_key_down",
        lua.create_function(move |_, _: Variadic<Value>| -> mlua::Result<bool> {
            if !legacy_api_reported.swap(true, Ordering::Relaxed) {
                tracing::error!(
                    target: "shroudforge::mod",
                    mod_id = %legacy_mod_id,
                    "{LEGACY_KEY_DOWN_MIGRATION_MESSAGE}"
                );
            }
            Err(mlua::Error::runtime(LEGACY_KEY_DOWN_MIGRATION_MESSAGE))
        })?,
    )?;
    let keybind_settings = r#mod.info().settings.clone();
    let keybind_id = mod_id.clone();
    let declared_keybinds = keybind_settings.clone();
    input.raw_set(
        "is_keybind_down",
        lua.create_function(move |lua, key: String| {
            let binding = read_keybind(lua, &keybind_id, &declared_keybinds, &key)?;
            Ok(binding.is_some_and(|binding| {
                is_game_focused()
                    && !shroudforge_hotkeys::is_capture_active()
                    && shroudforge_hotkeys::is_hotkey_down(&binding)
            }))
        })?,
    )?;
    let keybind_id = mod_id.clone();
    let press_keybind_settings = keybind_settings.clone();
    let press_revisions = Arc::new(Mutex::new(HashMap::<String, (String, u64)>::new()));
    input.raw_set(
        "was_keybind_pressed",
        lua.create_function(move |lua, key: String| {
            let binding = read_keybind(lua, &keybind_id, &press_keybind_settings, &key)?;
            let Some(binding) = binding else {
                if let Ok(mut revisions) = press_revisions.lock() {
                    revisions.remove(&key);
                }
                return Ok(false);
            };
            let signature = binding.normalized();
            let revision = shroudforge_hotkeys::hotkey_press_revision(&binding);
            let focused = is_game_focused() && !shroudforge_hotkeys::is_capture_active();
            let Ok(mut revisions) = press_revisions.lock() else {
                return Ok(false);
            };
            match revisions.get_mut(&key) {
                Some((previous_signature, previous_revision))
                    if *previous_signature == signature =>
                {
                    let pressed = focused && revision > *previous_revision;
                    *previous_revision = revision;
                    Ok(pressed)
                }
                _ => {
                    revisions.insert(key, (signature, revision));
                    Ok(false)
                }
            }
        })?,
    )?;
    let keybind_id = mod_id.clone();
    input.raw_set(
        "keybind_label",
        lua.create_function(move |lua, key: String| {
            Ok(read_keybind(lua, &keybind_id, &keybind_settings, &key)?
                .map(|binding| binding.label()))
        })?,
    )?;
    table.raw_set("input", input)?;

    let ui = lua.create_table()?;
    let ui_mod_id = mod_id.clone();
    ui.raw_set(
        "on_action",
        lua.create_function(move |lua, (action, callback): (String, Function)| {
            if !valid_identifier(&ui_mod_id) || !valid_identifier(&action) {
                return Err(mlua::Error::runtime("invalid ui action"));
            }
            lua.set_named_registry_value(&action_registry_key(&ui_mod_id, &action), callback)
        })?,
    )?;
    ui.raw_set(
        "toggle_modloader",
        lua.create_function(|_, ()| request_modloader_toggle().map_err(mlua::Error::runtime))?,
    )?;
    table.raw_set("ui", ui)?;

    // Reads the config file at each call: call it when settings change, not every update.
    let locale_dir = game_dir.clone();
    table.raw_set(
        "locale",
        lua.create_function(move |_, ()| Ok(read_ui_locale(&locale_dir)))?,
    )?;

    let locales = r#mod.info().locales.clone();
    let i18n = lua.create_table()?;
    i18n.raw_set(
        "get",
        lua.create_function(
            move |_, (key, locale, fallback): (String, String, Option<String>)| {
                Ok(mod_loader::localized_string(
                    &locales,
                    &key,
                    &locale,
                    fallback.as_deref(),
                ))
            },
        )?,
    )?;
    table.raw_set("i18n", i18n)?;

    let notifications = lua.create_table()?;
    notifications.raw_set(
        "publish",
        lua.create_function(move |_, notice: Table| {
            let id = notice.get::<String>("id")?;
            let title = notice.get::<String>("title")?;
            let message = notice.get::<String>("message")?;
            let level = notice.get::<Option<String>>("level")?.unwrap_or_else(|| "info".into());
            let action_url = notice.get::<Option<String>>("action_url")?;
            if !valid_identifier(&mod_id)
                || !valid_identifier(&id)
                || title.len() > 120
                || message.len() > 2_000
            {
                return Err(mlua::Error::runtime("invalid notification payload"));
            }
            if !matches!(level.as_str(), "info" | "success" | "warning" | "error" | "update") {
                return Err(mlua::Error::runtime("invalid notification level"));
            }
            if action_url.as_deref().is_some_and(|url| !url.starts_with("https://")) {
                return Err(mlua::Error::runtime("notification action_url must use HTTPS"));
            }
            let namespaced_id = format!("{mod_id}.{id}");
            let payload = json!({
                "id": namespaced_id,
                "modId": mod_id,
                "title": title,
                "message": message,
                "level": level,
                "actionUrl": action_url,
                "updatedAt": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
            });
            shroudforge_news::write_event(&game_dir, &format!("{mod_id}-{id}"), &payload).map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;
    table.raw_set("notifications", notifications)?;

    Ok(table)
}

fn read_keybind(
    lua: &mlua::Lua,
    mod_id: &str,
    declared: &[mod_loader::SettingDefinition],
    key: &str,
) -> mlua::Result<Option<shroudforge_hotkeys::Hotkey>> {
    if !declared.iter().any(|setting| {
        setting.key == key && matches!(setting.control, Some(mod_loader::SettingControl::Keybind))
    }) {
        return Err(mlua::Error::runtime("setting is not a declared keybind"));
    }
    let current: Table = lua.named_registry_value(&format!("shroudforge.settings.{mod_id}"))?;
    let mut controller_values = serde_json::Map::new();
    for condition in declared
        .iter()
        .filter_map(|setting| setting.enabled_when.as_ref())
    {
        controller_values.insert(
            condition.setting.clone(),
            read_setting_value(&current, &condition.setting)?,
        );
    }
    if !mod_loader::setting_is_enabled(key, declared, &serde_json::Value::Object(controller_values))
    {
        return Ok(None);
    }
    shroudforge_hotkeys::parse_value(&read_setting_value(&current, key)?)
        .map_err(mlua::Error::runtime)
}

fn read_setting_value(current: &Table, key: &str) -> mlua::Result<serde_json::Value> {
    Ok(match current.raw_get::<Value>(key)? {
        Value::Boolean(value) => serde_json::Value::Bool(value),
        Value::Integer(value) => json!(value),
        Value::Number(value) => json!(value),
        Value::String(value) => json!(value.to_str()?.to_string()),
        _ => serde_json::Value::Null,
    })
}

fn is_game_focused() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            System::Threading::GetCurrentProcessId,
            UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        };

        let foreground = unsafe { GetForegroundWindow() };
        if foreground.is_null() {
            return false;
        }
        let mut process_id = 0;
        unsafe { GetWindowThreadProcessId(foreground, &mut process_id) };
        return process_id == unsafe { GetCurrentProcessId() };
    }
    #[cfg(not(windows))]
    {
        false
    }
}

const LEGACY_KEY_DOWN_MIGRATION_MESSAGE: &str = concat!(
    "shroudforge.input.is_key_down was removed and is unsupported in ShroudForge 1.7.6. ",
    "Declare a control: \"keybind\" setting in extended.mod.json, assign it in the ",
    "Modloader Hotkey Manager under Compatibility > Hotkeys, and use ",
    "shroudforge.input.was_keybind_pressed(settingKey) for presses or ",
    "shroudforge.input.is_keybind_down(settingKey) while held."
);

#[cfg(windows)]
fn request_modloader_toggle() -> Result<(), String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        EVENT_MODIFY_STATE, GetCurrentProcessId, OpenEventW, SetEvent,
    };

    let name = format!(
        "Local\\ShroudForge.ModMenu.Open.{}",
        unsafe { GetCurrentProcessId() }
    );
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
    if event.is_null() {
        return Err(format!(
            "Modloader UI toggle bridge is unavailable: {}",
            std::io::Error::last_os_error()
        ));
    }
    let signaled = unsafe { SetEvent(event) } != 0;
    unsafe { CloseHandle(event) };
    if signaled {
        Ok(())
    } else {
        Err(format!(
            "Could not signal Modloader UI toggle: {}",
            std::io::Error::last_os_error()
        ))
    }
}

#[cfg(not(windows))]
fn request_modloader_toggle() -> Result<(), String> {
    Err("Modloader UI toggle is only available on Windows clients".into())
}

pub(super) fn action_registry_key(mod_id: &str, action: &str) -> String {
    format!("shroudforge.ui.action.{mod_id}.{action}")
}

pub(super) fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}
