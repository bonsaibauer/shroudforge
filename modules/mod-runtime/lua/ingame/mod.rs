use kfc::reflection::{PrimitiveType, TypeMetadata, TypeRegistry};
use mod_loader::{Capability, Mod};
use shroudforge_compatibility::Availability;
use std::rc::Rc;

use crate::{
    RuntimePhase,
    env::{
        AppFeatures, AppState,
        game::value::{
            convert_lua_to_value, convert_value_to_lua, mapped_source_bytes,
            validate_and_clone_lua_value,
        },
        registration::add_function_with_mod,
    },
    lua::{FunctionArgs, LuaError, LuaValue},
    type_aliases::MappedValue,
};

use crate::env::{loader, registration, registry, runtime_ownership, runtime_values};
pub(crate) use crate::runtime_provider;

mod check_ingame_access;
use check_ingame_access::*;
pub(crate) use check_ingame_access::{available, runtime_denial_reason};
pub(crate) mod access_named_service;
pub(crate) mod ecs;
pub(crate) mod functions;
mod impact;
pub(crate) mod overrides;
pub(crate) mod player_input;
pub(crate) mod prove_buffer_transform;
pub(crate) mod resources;
mod set_profile_patch_enabled;
pub(crate) mod world;
use ecs::create_ecs;
use functions::create_functions;
use set_profile_patch_enabled::*;
use world::create_world;

pub(crate) fn runtime_provider_report() -> serde_json::Value {
    runtime_provider::report()
}

pub fn create(lua: &mlua::Lua, r#mod: Mod) -> mlua::Result<mlua::Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();
    crate::session::configure_component_contract::configure_component_contract(&app_state);
    let table = lua.create_table()?;
    table.raw_set("phase", app_state.phase().as_str())?;
    table.raw_set("schema_version", 1)?;
    table.raw_set("is_client", app_state.is_client())?;
    table.raw_set("is_server", app_state.is_server())?;
    add_function_with_mod(lua, &table, "has", &r#mod, lua_has)?;
    add_function_with_mod(lua, &table, "require", &r#mod, lua_require)?;
    add_function_with_mod(lua, &table, "status", &r#mod, lua_status)?;
    add_function_with_mod(lua, &table, "get_operations", &r#mod, lua_get_operations)?;
    add_function_with_mod(lua, &table, "report_effect", &r#mod, lua_report_effect)?;

    let player = lua.create_table()?;
    add_function_with_mod(lua, &player, "name", &r#mod, lua_player_name)?;
    table.raw_set("player", player)?;
    table.raw_set("ecs", create_ecs(lua, &r#mod)?)?;
    table.raw_set("impact", impact::create(lua, &r#mod)?)?;
    table.raw_set(
        "services",
        crate::env::runtime_services::create(lua, &r#mod)?,
    )?;
    table.raw_set(
        "overrides",
        crate::env::runtime_overrides::create(lua, &r#mod)?,
    )?;
    table.raw_set("world", create_world(lua, &r#mod)?)?;
    table.raw_set(
        "network",
        crate::env::runtime_networking::create(lua, &r#mod)?,
    )?;
    table.raw_set("patch", create_patch(lua, &r#mod)?)?;
    table.raw_set("functions", create_functions(lua, &table, &r#mod)?)?;

    Ok(table)
}

fn lua_player_name(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(Option<String>, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.player.name") {
        return Ok((None, Some(reason)));
    }
    let name = runtime_provider::selected_character_name();
    let reason = name
        .is_none()
        .then(|| "selected character name is unavailable".to_owned());
    Ok((name, reason))
}

fn lua_vec<const N: usize>(table: mlua::Table, label: &str) -> mlua::Result<[f64; N]> {
    if table.raw_len() != N {
        return Err(LuaError::generic(format!(
            "{label} must contain exactly {N} numbers"
        )));
    }
    let mut values = [0.0; N];
    for (index, value) in values.iter_mut().enumerate() {
        *value = table.raw_get(index + 1)?;
    }
    Ok(values)
}

pub(crate) fn runtime_type_name(
    lua: &mlua::Lua,
    args: &FunctionArgs,
    index: usize,
) -> mlua::Result<String> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let type_index =
        crate::env::registry::resolve(state.type_registry(), args.get::<LuaValue>(index)?)
            .map_err(LuaError::generic)?;
    let r#type = state.type_registry().get(type_index).unwrap();

    if !r#type.qualified_name.starts_with("keen::ecs::") {
        return Err(LuaError::generic(format!(
            "runtime ECS operation requires a keen::ecs::* type, got {}",
            r#type.qualified_name
        )));
    }

    Ok(runtime_provider::resolve_type(&r#type.qualified_name)
        .unwrap_or_else(|| r#type.qualified_name.clone()))
}
