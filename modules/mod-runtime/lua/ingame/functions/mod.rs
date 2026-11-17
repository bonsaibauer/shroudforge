use super::*;
use crate::lua::convert_json::json_to_lua;
use mlua::{Lua, Table};
use std::{cell::RefCell, rc::Rc};
mod bind_profile_modifier;
mod read_function_catalog;
pub(crate) use read_function_catalog::attach;
use read_function_catalog::{Catalog, descriptor_to_lua};

pub(super) fn create_functions(
    lua: &mlua::Lua,
    runtime: &mlua::Table,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let result = lua.create_table()?;
    add_function_with_mod(lua, &result, "get_operations", r#mod, lua_get_operations)?;
    crate::env::runtime_functions::attach(lua, &result, &runtime.raw_get::<mlua::Table>("patch")?)?;
    // Capture existing checked wrappers. No second calling convention/dispatcher.
    let bindings = lua.create_table()?;
    let catalog = crate::lua::shroudforge::v1::read_runtime_operations::catalog()
        .map_err(LuaError::generic)?;
    for id in catalog["runtimeOperations"].as_array().unwrap() {
        let id = id.as_str().unwrap();
        let path = match id {
            "runtime.world.context.active" => "runtime.world.context_active",
            "runtime.world.voxel.grid_spec" => "runtime.world.voxel.get_grid_spec",
            "runtime.network.status" => "runtime.network.status",
            "runtime.network.send" => "runtime.network.send",
            "runtime.network.send_mod" => "runtime.network.send_mod",
            "runtime.network.receive" => "runtime.network.receive",
            "runtime.network.receive_mod" => "runtime.network.receive_mod",
            "runtime.network.accept" => "runtime.network.accept",
            "runtime.network.connected_peers" => "runtime.network.connected_peers",
            "runtime.network.player_permissions" => "runtime.network.player_permissions",
            "runtime.lifecycle" | "runtime.gameplay.patch" => continue,
            id => id,
        };
        let mut value = LuaValue::Table(runtime.clone());
        for part in path.split('.').skip(1) {
            value = match value {
                LuaValue::Table(table) => table.raw_get::<LuaValue>(part)?,
                _ => LuaValue::Nil,
            };
        }
        if matches!(value, LuaValue::Function(_)) {
            bindings.raw_set(id, value)?;
        }
    }
    let native_get = result.raw_get::<mlua::Function>("get")?;
    let operation_get = native_get.clone();
    let operation_bindings = bindings.clone();
    let operation_mod = r#mod.clone();
    result.raw_set(
        "get",
        lua.create_function(move |lua, selector: LuaValue| {
            let (descriptor, reason): (Option<mlua::Table>, Option<String>) =
                operation_get.call(selector.clone())?;
            if let (Some(descriptor), LuaValue::String(id)) = (&descriptor, &selector) {
                let id = id.to_str()?;
                if let Some(call) =
                    operation_bindings.raw_get::<Option<mlua::Function>>(id.as_ref())?
                {
                    let state = lua.app_data_ref::<AppState>().unwrap();
                    let ready = runtime_denial_reason(&state, &operation_mod, &id).is_none()
                        && available(&state, &operation_mod, &id);
                    descriptor.raw_set("operation_id", id.as_ref())?;
                    descriptor.raw_set("callable", ready)?;
                    descriptor.raw_set("execution", "checked-runtime-operation")?;
                    descriptor.raw_set("call", call)?;
                    descriptor.raw_set(
                        "reason",
                        if ready {
                            None
                        } else {
                            Some("operation context/capability is currently unavailable")
                        },
                    )?;
                }
            }
            Ok((descriptor, reason))
        })?,
    )?;
    let r#mod = r#mod.clone();
    result.raw_set(
        "bind",
        lua.create_function(move |lua, id: String| {
            let state = lua.app_data_ref::<AppState>().unwrap();
            let function = bindings.raw_get::<Option<mlua::Function>>(id.as_str())?;
            if function.is_none() {
                let (descriptor, reason): (Option<mlua::Table>, Option<String>) =
                    native_get.call(id.clone())?;
                let Some(descriptor) = descriptor else {
                    return Ok((None, reason));
                };
                if descriptor.raw_get::<bool>("callable")? {
                    return Ok((Some(descriptor.raw_get::<mlua::Function>("call")?), None));
                }
                return Ok((None, descriptor.raw_get::<Option<String>>("reason")?));
            }
            if let Some(reason) = runtime_denial_reason(&state, &r#mod, &id) {
                return Ok((None, Some(reason)));
            }
            if !available(&state, &r#mod, &id) {
                return Ok((
                    None,
                    Some(format!("operation is currently unavailable: {id}")),
                ));
            }
            Ok((function, None))
        })?,
    )?;
    Ok(result)
}
