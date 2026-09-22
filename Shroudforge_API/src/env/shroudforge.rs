use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use mlua::{Function, Table, Value, Variadic};
use mod_loader::Mod;
use serde_json::json;

use crate::lua::LuaValue;

pub fn create(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.raw_set("version", env!("CARGO_PKG_VERSION"))?;
    table.raw_set("mod_id", r#mod.info().id.as_str())?;
    table.raw_set("mod_kind", "lua")?;

    let log = lua.create_table()?;
    for level in ["debug", "info", "warn", "error"] {
        let id = r#mod.info().id.clone();
        log.raw_set(
            level,
            lua.create_function(move |_, args: Variadic<LuaValue>| {
                let message = args
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect::<mlua::Result<Vec<_>>>()?
                    .join("\t");
                match level {
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
    let mod_id = r#mod.info().id.clone();

    let settings = lua.create_table()?;
    let settings_root = game_dir.clone();
    let settings_mod_id = mod_id.clone();
    let setting_defaults = r#mod
        .info()
        .settings
        .iter()
        .map(|setting| (setting.key.clone(), setting.default.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    settings.raw_set(
        "get",
        lua.create_function(move |lua, (key, fallback): (String, Option<Value>)| {
            if !valid_identifier(&settings_mod_id) || !valid_identifier(&key) {
                return Err(mlua::Error::runtime("invalid setting key"));
            }
            let path = settings_root.join("config").join("shroudforge.json");
            let value = fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|document| {
                    document
                        .get("mods")?
                        .get(&settings_mod_id)?
                        .get("settings")?
                        .get(&key)
                        .cloned()
                })
                .or_else(|| setting_defaults.get(&key).cloned());
            match value {
                Some(value) => json_to_lua(lua, &value),
                None => Ok(fallback.unwrap_or(Value::Nil)),
            }
        })?,
    )?;
    table.raw_set("settings", settings)?;

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
    table.raw_set("ui", ui)?;

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
            let directory = game_dir.join("Shroudforge_UI").join("notifications");
            fs::create_dir_all(&directory).map_err(mlua::Error::external)?;
            let path = directory.join(format!("{mod_id}-{id}.json"));
            let temporary = path.with_extension("tmp");
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
            fs::write(&temporary, serde_json::to_vec_pretty(&payload).map_err(mlua::Error::external)?)
                .map_err(mlua::Error::external)?;
            if path.exists() { fs::remove_file(&path).map_err(mlua::Error::external)?; }
            fs::rename(temporary, path).map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;
    table.raw_set("notifications", notifications)?;

    Ok(table)
}

fn action_registry_key(mod_id: &str, action: &str) -> String {
    format!("shroudforge.ui.action.{mod_id}.{action}")
}

pub(crate) fn dispatch_ui_actions(lua: &mlua::Lua) {
    let Some(game_dir) = lua
        .app_data_ref::<crate::env::AppState>()
        .map(|state| state.env().game_dir().as_std_path().to_path_buf())
    else {
        return;
    };
    let directory = game_dir.join("Shroudforge_UI").join("actions");
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten().take(32) {
        let path = entry.path();
        let Some(value) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        else {
            let _ = fs::remove_file(path);
            continue;
        };
        let Some(mod_id) = value.get("modId").and_then(|value| value.as_str()) else {
            let _ = fs::remove_file(path);
            continue;
        };
        let Some(action) = value.get("action").and_then(|value| value.as_str()) else {
            let _ = fs::remove_file(path);
            continue;
        };
        if !valid_identifier(mod_id) || !valid_identifier(action) {
            let _ = fs::remove_file(path);
            continue;
        }
        match lua.named_registry_value::<Function>(&action_registry_key(mod_id, action)) {
            Ok(callback) => {
                if let Err(error) = callback.call::<()>(()) {
                    tracing::error!(target: "shroudforge::mod", mod_id, action, "ui action failed: {error}");
                }
                let _ = fs::remove_file(path);
            }
            Err(_) => {
                // Keep the request until the runtime mod has registered its action.
            }
        }
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn json_to_lua<'lua>(lua: &'lua mlua::Lua, value: &serde_json::Value) -> mlua::Result<Value> {
    Ok(match value {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(value) => Value::Boolean(*value),
        serde_json::Value::Number(value) if value.is_i64() => {
            Value::Integer(value.as_i64().unwrap())
        }
        serde_json::Value::Number(value) => Value::Number(value.as_f64().unwrap_or_default()),
        serde_json::Value::String(value) => Value::String(lua.create_string(value)?),
        serde_json::Value::Array(values) => {
            let table = lua.create_table_with_capacity(values.len(), 0)?;
            for (index, value) in values.iter().enumerate() {
                table.raw_set(index + 1, json_to_lua(lua, value)?)?;
            }
            Value::Table(table)
        }
        _ => {
            return Err(mlua::Error::runtime(
                "setting value must be an array, string, number, boolean or null",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use mlua::Value;

    use super::json_to_lua;

    #[test]
    fn converts_json_arrays_to_lua_sequences() {
        let lua = mlua::Lua::new();
        let input = serde_json::json!(["One", "Half", "CustomAmount"]);
        let Value::Table(table) = json_to_lua(&lua, &input).unwrap() else {
            panic!("array setting did not become a Lua table");
        };

        assert_eq!(table.raw_get::<String>(1).unwrap(), "One");
        assert_eq!(table.raw_get::<String>(2).unwrap(), "Half");
        assert_eq!(table.raw_get::<String>(3).unwrap(), "CustomAmount");
    }
}
