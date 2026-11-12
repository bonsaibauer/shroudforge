//! Flight authority is bound to the registered bundled mod and generation.
use super::{AppState, Capability, FunctionArgs, Mod, RuntimePhase, has_capability};
use crate::env::registration::add_function_with_mod;
pub(super) fn create(lua: &mlua::Lua, owner: &Mod) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    add_function_with_mod(lua, &table, "set_local_enabled", owner, set)?;
    add_function_with_mod(lua, &table, "reset_local", owner, reset)?;
    add_function_with_mod(lua, &table, "status", owner, status)?;
    Ok(table)
}
fn token(state: &AppState, owner: &Mod) -> Result<u64, String> {
    if owner.info().id != "sf-unlimited-flight"
        || state.phase() != RuntimePhase::Ingame
        || !state.is_client()
        || state.is_server()
        || !has_capability(owner, Capability::Runtime)
        || !state.runtime_mod_is_active(&owner.info().id)
    {
        return Err("Flight requires the active bundled client mod in the ingame phase".into());
    }
    Ok(state.lua_map_owner(&owner.info().id))
}
fn set(lua: &mlua::Lua, args: FunctionArgs, owner: &Mod) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let token = match token(&state, owner) {
        Ok(v) => v,
        Err(e) => return Ok((false, Some(e))),
    };
    let enabled = args.get::<bool>(0)?;
    let result = shroudforge_runtime_bridge::flight_set_enabled(token, enabled);
    Ok((
        result == 1,
        (result != 1)
            .then(|| "Qualified Flight provider is unavailable or ownership was refused".into()),
    ))
}
fn reset(lua: &mlua::Lua, _args: FunctionArgs, owner: &Mod) -> mlua::Result<bool> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    Ok(
        token(&state, owner)
            .is_ok_and(|token| shroudforge_runtime_bridge::flight_reset(token) == 1),
    )
}
fn status(lua: &mlua::Lua, _args: FunctionArgs, owner: &Mod) -> mlua::Result<mlua::Table> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let result = lua.create_table()?;
    let status = token(&state, owner)
        .ok()
        .and_then(shroudforge_runtime_bridge::flight_status);
    result.set("requested_on", status.is_some_and(|s| s.flags & 4 != 0))?;
    result.set("local_ready", status.is_some_and(|s| s.flags & 1 != 0))?;
    result.set("provider_ready", status.is_some_and(|s| s.flags & 2 != 0))?;
    result.set("server_acknowledged", false)?;
    Ok(result)
}
