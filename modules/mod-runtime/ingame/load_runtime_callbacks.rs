use super::Lifecycle;
use crate::runner::LuaModRunner;
use mlua::{RegistryKey, Value};
use mod_loader::ModEnvironment;

pub(super) fn load_runtime_callbacks(
    env: &ModEnvironment,
    runner: &LuaModRunner,
    diagnostics: &mut shroudforge_runtime_diagnostics::Session,
    errors: &mut serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<Vec<Lifecycle>> {
    let mut lifecycle = Vec::new();
    for id in runner.runtime_mod_ids() {
        tracing::debug!(target: "shroudforge::runtime", mod_id = %id,
        "Loading runtime mod entrypoint");
        let module_result = runner.load_runtime_module(&id);
        let value = match module_result {
            Ok(value) => {
                tracing::debug!(target: "shroudforge::runtime", mod_id = %id,
                "Runtime mod entrypoint loaded");
                value
            }
            Err(error) => {
                errors.insert(
                    id.clone(),
                    format!("module initialization failed: {error}").into(),
                );
                tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                "module initialization failed. Scope discarded: {error}");
                continue;
            }
        };
        let table = match value {
            Value::Table(table) => Some(table),
            Value::Nil => None,
            _ => {
                tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                "entrypoint must return a lifecycle table or nil");
                errors.insert(
                    id.clone(),
                    "entrypoint must return a lifecycle table or nil".into(),
                );
                continue;
            }
        };
        let update_interval_ms = match table.as_ref() {
            Some(table) => table.get::<Option<u64>>("update_interval_ms"),
            None => Ok(None),
        };
        let update_interval_ms = match update_interval_ms {
            Ok(Some(value)) if (8..=1000).contains(&value) => value,
            Ok(None) => 50,
            Ok(Some(_)) | Err(_) => {
                errors.insert(
                    id.clone(),
                    "update_interval_ms must be an integer from 8 to 1000".into(),
                );
                continue;
            }
        };
        let (load, update, unload) = match (
            callback(&runner.lua, table.as_ref(), "on_load"),
            callback(&runner.lua, table.as_ref(), "on_update"),
            callback(&runner.lua, table.as_ref(), "on_unload"),
        ) {
            (Ok(load), Ok(update), Ok(unload)) => (load, update, unload),
            _ => {
                tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                "invalid lifecycle. Scope discarded");
                errors.insert(id.clone(), "invalid lifecycle".into());
                continue;
            }
        };
        let configured_enabled = env
            .mod_registry()
            .get(&id)
            .is_some_and(|item| item.info().enabled);
        if configured_enabled {
            let failed_dependency = env
                .mod_registry()
                .get(&id)
                .and_then(|item| {
                    item.info().dependencies.iter().find(|dependency| {
                        !dependency.optional.unwrap_or(false)
                            && dependency.id != "shroudforge-api"
                            && dependency.id != mod_loader::SHROUDFORGE_RELEASE_DEPENDENCY
                            && errors.contains_key(&dependency.id)
                    })
                })
                .map(|dependency| dependency.id.clone());
            if let Some(dependency) = failed_dependency {
                errors.insert(
                    id.clone(),
                    format!(
                        "dependency-blocked: required dependency failed to initialize: {dependency}"
                    )
                    .into(),
                );
            }
        }
        let enabled = configured_enabled && !errors.contains_key(&id);
        if configured_enabled && !enabled {
            tracing::warn!(target: "shroudforge::runtime", mod_id = %id,
            "Runtime mod is loaded for possible live activation but remains inactive because a dependency is blocked");
        }
        let mut item = Lifecycle {
            id: id.clone(),
            load,
            update,
            unload,
            update_interval: std::time::Duration::from_millis(update_interval_ms),
            next_update: std::time::Instant::now(),
            active: enabled,
            cleanup_pending: false,
        };
        if enabled {
            let result = item.activate_runtime_mod(&runner.lua, Some(diagnostics))?;
            if let Err(error) = result {
                tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                "on_load failed. Rolling back scope: {error}");
                errors.insert(id.clone(), format!("on_load failed: {error}").into());
                if let Err(cleanup) = item.rollback_failed_activation(&runner.lua) {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                    "Rollback is pending and will be retried: {cleanup}");
                } else {
                    continue;
                }
            }
        }
        if enabled && !item.cleanup_pending {
            tracing::info!(target: "shroudforge::runtime", mod_id = %id,
            "Runtime mod initialized and active");
        }
        lifecycle.push(item);
    }

    Ok(lifecycle)
}

fn callback(
    lua: &mlua::Lua,
    lifecycle: Option<&mlua::Table>,
    name: &str,
) -> mlua::Result<Option<RegistryKey>> {
    let value = if let Some(table) = lifecycle {
        table.get::<Value>(name)?
    } else {
        Value::Nil
    };
    match value {
        Value::Nil => Ok(None),
        Value::Function(value) => Ok(Some(lua.create_registry_value(value)?)),
        _ => Err(mlua::Error::runtime(format!("{name} must be a function"))),
    }
}
