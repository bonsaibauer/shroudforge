use super::*;
use crate::lua::convert_json::json_to_lua;
fn authority(lua: &mlua::Lua, owner: &Mod, feature: &str) -> Result<u64, String> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, owner, feature) {
        return Err(reason);
    }
    if state.is_server() || !owner.info().target.supports_process(false) {
        return Err("map services require a client package and process".into());
    }
    Ok(state.lua_map_owner(&owner.info().id))
}
fn unavailable(status: i32) -> String {
    format!(
        "native map service unavailable (status {status}); requires an admitted client build and current capture"
    )
}
pub(crate) fn create(lua: &mlua::Lua, owner: &Mod) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    add_function_with_mod(lua, &table, "get_frame", owner, frame)?;
    add_function_with_mod(lua, &table, "get_status", owner, status)?;
    add_function_with_mod(lua, &table, "get_markers", owner, markers)?;
    add_function_with_mod(lua, &table, "get_fog", owner, fog)?;
    Ok(table)
}
fn status(
    lua: &mlua::Lua,
    args: FunctionArgs,
    owner: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    if args.len() != 0 {
        return Err(LuaError::generic("map.get_status takes no arguments"));
    }
    let generation = match authority(lua, owner, "runtime.map.status") {
        Ok(value) => value,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    match shroudforge_runtime_bridge::map_overlay_status(&owner.info().id, generation) {
        Ok(value) => Ok((json_to_lua(lua, &value)?, None)),
        Err(status) => Ok((LuaValue::Nil, Some(unavailable(status)))),
    }
}
fn frame(
    lua: &mlua::Lua,
    args: FunctionArgs,
    owner: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    if args.len() != 0 {
        return Err(LuaError::generic("map.get_frame takes no arguments"));
    }
    let generation = match authority(lua, owner, "runtime.map.frame") {
        Ok(value) => value,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    match shroudforge_runtime_bridge::map_summary(&owner.info().id, generation) {
        Ok(value) => Ok((json_to_lua(lua, &value)?, None)),
        Err(status) => Ok((LuaValue::Nil, Some(unavailable(status)))),
    }
}
fn optional_u64(args: &FunctionArgs, index: usize) -> mlua::Result<u64> {
    if args.len() <= index {
        return Ok(0);
    }
    let value = args.get::<String>(index)?;
    value
        .parse::<u64>()
        .map_err(|_| LuaError::generic("generation/revision must be a decimal u64 string"))
}
fn markers(
    lua: &mlua::Lua,
    args: FunctionArgs,
    owner: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    if args.len() < 3 || args.len() > 4 {
        return Err(LuaError::generic(
            "map.get_markers(collection, offset, limit, generation?)",
        ));
    }
    let collection = match args.get::<String>(0)?.as_str() {
        "world" => 0,
        "players" => 1,
        "deaths" => 2,
        "waypoints" => 3,
        "pings" => 4,
        "discovery" => 5,
        _ => return Err(LuaError::generic("unknown map marker collection")),
    };
    let offset = args.get::<u32>(1)?;
    let limit = args.get::<u32>(2)?;
    let expected = optional_u64(&args, 3)?;
    let generation = match authority(lua, owner, "runtime.map.markers") {
        Ok(value) => value,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    match shroudforge_runtime_bridge::map_markers(
        &owner.info().id,
        generation,
        collection,
        offset,
        limit,
        expected,
    ) {
        Ok(value) => Ok((json_to_lua(lua, &value)?, None)),
        Err(status) => Ok((LuaValue::Nil, Some(unavailable(status)))),
    }
}
fn fog(
    lua: &mlua::Lua,
    args: FunctionArgs,
    owner: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    if args.len() < 2 || args.len() > 4 {
        return Err(LuaError::generic(
            "map.get_fog(offset, count, generation?, revision?)",
        ));
    }
    let offset = args.get::<u32>(0)?;
    let count = args.get::<u32>(1)?;
    let expected = optional_u64(&args, 2)?;
    let revision = optional_u64(&args, 3)?;
    let generation = match authority(lua, owner, "runtime.map.fog") {
        Ok(value) => value,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    match shroudforge_runtime_bridge::map_fog(
        &owner.info().id,
        generation,
        u64::from(offset),
        count,
        expected,
        revision,
    ) {
        Ok((meta, bytes)) => {
            let table = lua.create_table()?;
            table.raw_set("meta", json_to_lua(lua, &meta)?)?;
            table.raw_set("bytes", lua.create_string(bytes)?)?;
            Ok((LuaValue::Table(table), None))
        }
        Err(status) => Ok((LuaValue::Nil, Some(unavailable(status)))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::FromLuaMulti;
    #[test]
    fn snapshot_tokens_remain_lossless_and_reject_invalid_values() -> mlua::Result<()> {
        let lua = mlua::Lua::new();
        let args = FunctionArgs::from_lua_multi(
            mlua::MultiValue::from_vec(vec![LuaValue::String(
                lua.create_string("18446744073709551615")?,
            )]),
            &lua,
        )?;
        assert_eq!(optional_u64(&args, 0)?, u64::MAX);
        assert_eq!(optional_u64(&args, 1)?, 0);
        let invalid = FunctionArgs::from_lua_multi(
            mlua::MultiValue::from_vec(vec![LuaValue::String(
                lua.create_string("18446744073709551616")?,
            )]),
            &lua,
        )?;
        assert!(optional_u64(&invalid, 0).is_err());
        Ok(())
    }
}
