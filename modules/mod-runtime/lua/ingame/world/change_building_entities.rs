use super::*;

pub(super) fn lua_world_entity_spawn(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.spawn") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    let spawn = read_spawn_arguments(&args, "runtime.world.entity.spawn")?;
    match runtime_provider::world_entity_spawn(
        spawn.template_uuid,
        spawn.position,
        spawn.rotation,
        spawn.tracking,
        spawn.flags,
    ) {
        Ok(token) => Ok((LuaValue::Integer(i64::from(token)), None)),
        Err(reason) => Ok((LuaValue::Nil, Some(reason))),
    }
}

/// The arguments of a prop spawn, shared by `spawn` and `start_spawn`.
pub(super) struct SpawnArguments {
    pub(super) template_uuid: [u64; 2],
    pub(super) position: [f64; 3],
    pub(super) rotation: [f64; 4],
    pub(super) tracking: u32,
    pub(super) flags: u32,
}

pub(super) fn read_spawn_arguments(args: &FunctionArgs, name: &str) -> mlua::Result<SpawnArguments> {
    if args.len() != 6 {
        return Err(LuaError::generic(format!(
            "{name} expects templateUuidHighHex, templateUuidLowHex, position[3], rotation[4], trackingId, flags",
        )));
    }
    let high =
        u64::from_str_radix(args.get::<String>(0)?.trim_start_matches("0x"), 16).map_err(|_| {
            LuaError::generic("template UUID high must be a 16-digit hexadecimal string")
        })?;
    let low =
        u64::from_str_radix(args.get::<String>(1)?.trim_start_matches("0x"), 16).map_err(|_| {
            LuaError::generic("template UUID low must be a 16-digit hexadecimal string")
        })?;
    Ok(SpawnArguments {
        template_uuid: [high, low],
        position: lua_vec::<3>(args.get::<mlua::Table>(2)?.clone(), "position")?,
        rotation: lua_vec::<4>(args.get::<mlua::Table>(3)?.clone(), "rotation")?,
        tracking: args.get::<u32>(4)?,
        flags: args.get::<u32>(5)?,
    })
}

pub(super) fn lua_world_entity_place(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    lua_world_entity_placement(lua, args, r#mod, false)
}

pub(super) fn lua_world_entity_destroy(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    if args.len() == 1 {
        let state = lua.app_data_ref::<AppState>().unwrap();
        if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.destroy") {
            return Ok((false, Some(reason)));
        }
        let handle = args.get::<u32>(0)?;
        return match runtime_provider::world_entity_destroy_handle(handle) {
            Ok(()) => Ok((true, None)),
            Err(reason) => Ok((false, Some(reason))),
        };
    }
    lua_world_entity_placement(lua, args, r#mod, true)
}

pub(super) fn lua_world_entity_placement(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
    destroy: bool,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let operation = if destroy {
        "runtime.world.entity.destroy"
    } else {
        "runtime.world.entity.place"
    };
    if let Some(reason) = runtime_denial_reason(&state, r#mod, operation) {
        return Ok((false, Some(reason)));
    }
    if let Some(reason) =
        runtime_denial_reason(&state, r#mod, "runtime.world.entity.finish_building")
    {
        return Ok((false, Some(reason)));
    }
    if args.len() != 5 {
        return Err(LuaError::generic(
            "runtime.world.entity.place/destroy expects position[3], rotation[4], bounds[6], trackingId, feedbackId",
        ));
    }
    let position = lua_vec::<3>(args.get::<mlua::Table>(0)?.clone(), "position")?;
    let rotation = lua_vec::<4>(args.get::<mlua::Table>(1)?.clone(), "rotation")?;
    let bounds =
        lua_vec::<6>(args.get::<mlua::Table>(2)?.clone(), "bounds")?.map(|value| value as f32);
    let tracking = args.get::<u32>(3)?;
    let feedback = args.get::<u32>(4)?;
    match runtime_provider::world_entity_placement(
        destroy, position, rotation, bounds, tracking, feedback,
    ) {
        Ok(()) => Ok((true, None)),
        Err(reason) => Ok((false, Some(reason))),
    }
}

pub(super) fn lua_world_entity_finish_building(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) =
        runtime_denial_reason(&state, r#mod, "runtime.world.entity.finish_building")
    {
        return Ok((false, Some(reason)));
    }
    if args.len() != 1 {
        return Err(LuaError::generic(
            "runtime.world.entity.finish_building expects complete:boolean",
        ));
    }
    match runtime_provider::world_entity_finish_building(args.get::<bool>(0)?) {
        Ok(()) => Ok((true, None)),
        Err(reason) => Ok((false, Some(reason))),
    }
}
