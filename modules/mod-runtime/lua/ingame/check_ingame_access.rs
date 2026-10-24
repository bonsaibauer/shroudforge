use super::*;

pub(super) fn steam_network_ready() -> bool {
    let status = shroudforge_steam_networking::status();
    status.available && status.service_ready
}

pub(super) fn has_capability(r#mod: &Mod, capability: Capability) -> bool {
    r#mod.info().capabilities.contains(&capability)
}

pub(crate) fn available(state: &AppState, r#mod: &Mod, feature: &str) -> bool {
    match feature {
        "game.assets.write" => {
            state.has_feature(AppFeatures::PATCH) && has_capability(r#mod, Capability::Patch)
        }
        "export" => {
            state.has_feature(AppFeatures::EXPORT) && has_capability(r#mod, Capability::Export)
        }
        "runtime.lifecycle" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime("runtime.lifecycle")
                && has_capability(r#mod, Capability::Runtime)
        }
        "runtime.player.name" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::selected_character_name().is_some()
        }
        "runtime.is_main_menu" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::is_main_menu().is_some()
        }
        "runtime.ecs.query" | "runtime.ecs.resolve" | "runtime.ecs.read" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::ready()
        }
        "runtime.ecs.write" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::can_write()
        }
        "runtime.assets.write" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::resource_can_write()
        }
        "runtime.gameplay.patch" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::runtime_patch_available("runtime.gameplay.patch")
        }
        "runtime.network.status"
        | "runtime.network.send"
        | "runtime.network.send_mod"
        | "runtime.network.receive"
        | "runtime.network.receive_mod"
        | "runtime.network.accept"
        | "runtime.network.connected_peers" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && steam_network_ready()
        }
        "runtime.world.context.active" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_operation_available(feature)
                && runtime_provider::world_context_active()
        }
        "runtime.world.cursor.get" | "runtime.world.building.input" | "runtime.world.voxel.read" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_operation_available(feature)
        }
        "runtime.world.voxel.write" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_operation_available(feature)
                && runtime_provider::world_context_active()
        }
        "runtime.world.voxel.grid_spec" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_operation_available("runtime.world.voxel.read")
        }
        "runtime.world.entity.set_scale" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_entity_query_props_ready()
                && runtime_provider::can_write_component("keen::ecs::CurrentTransform")
        }
        "runtime.world.entity.destroy" | "runtime.world.entity.spawn" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_operation_available(feature)
                && runtime_provider::world_entity_query_props_ready()
        }
        "runtime.world.entity.query_props"
        | "runtime.world.entity.query_props_in_bounds"
        | "runtime.world.entity.register_prop_recipes"
        | "runtime.world.entity.get_transform"
        | "runtime.world.entity.place"
        | "runtime.world.entity.finish_building" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && if feature == "runtime.world.entity.query_props"
                    || feature == "runtime.world.entity.query_props_in_bounds"
                    || feature == "runtime.world.entity.register_prop_recipes"
                    || feature == "runtime.world.entity.get_transform"
                {
                    runtime_provider::world_entity_query_props_ready()
                } else {
                    runtime_provider::world_operation_available(feature)
                        && runtime_provider::world_entity_context_ready()
                }
        }
        _ => false,
    }
}

pub(super) fn lua_has(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<bool> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    Ok(available(&state, r#mod, &args.get::<String>(0)?))
}

pub(super) fn lua_status(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let feature = args.get::<String>(0)?;
    operation_status(lua, &feature, r#mod)
}

pub(super) fn lua_get_operations(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let catalog = crate::lua::shroudforge::v1::read_runtime_operations::catalog()
        .map_err(LuaError::generic)?;
    let operations: Vec<String> =
        serde_json::from_value(catalog["runtimeOperations"].clone()).map_err(LuaError::external)?;
    let result = lua.create_table()?;
    let state = lua.app_data_ref::<AppState>().unwrap();
    for name in operations {
        let status = operation_status(lua, &name, r#mod)?;
        // Include phase/provider readiness from has(), plus lifecycle access.
        // status() alone also describes operations supported in a later phase.
        let denial = runtime_denial_reason(&state, r#mod, &name);
        let ready = available(&state, r#mod, &name) && denial.is_none();
        status.raw_set("available", ready)?;
        if let Some(reason) = denial {
            status.raw_set("reason", reason)?;
        } else if !ready && status.raw_get::<Option<String>>("reason")?.is_none() {
            status.raw_set("reason", "runtime provider is not ready for this operation")?;
        }
        result.raw_set(name, status)?;
    }
    Ok(result)
}

pub(super) fn operation_status(
    lua: &mlua::Lua,
    feature: &str,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if !has_capability_for_feature(r#mod, &feature) {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: format!("mod '{}' lacks the required capability", r#mod.info().id),
            },
        );
    }
    let availability = state.api().runtime(&feature);
    if feature == "runtime.is_main_menu"
        && matches!(availability, Availability::Available)
        && runtime_provider::is_main_menu().is_none()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "main menu state is unknown or transitioning".into(),
            },
        );
    }
    if feature == "runtime.player.name"
        && matches!(availability, Availability::Available)
        && runtime_provider::selected_character_name().is_none()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "selected character name is unavailable".into(),
            },
        );
    }
    if feature.starts_with("runtime.ecs.")
        && matches!(availability, Availability::Available)
        && !runtime_provider::ready()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "KFC Runtime is waiting for the live Keen ECS world".into(),
            },
        );
    }
    if feature.starts_with("runtime.network.")
        && matches!(availability, Availability::Available)
        && !steam_network_ready()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "Steam Networking Messages transport or session callback is unavailable in this process".into(),
            },
        );
    }
    if feature == "runtime.ecs.write"
        && matches!(availability, Availability::Available)
        && !runtime_provider::can_write()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "KFC Runtime has no writable live component context".into(),
            },
        );
    }
    if feature == "runtime.assets.write"
        && matches!(availability, Availability::Available)
        && !runtime_provider::resource_can_write()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "KFC Runtime has no validated live resource scalar write context".into(),
            },
        );
    }
    let world_ready =
        if feature == "runtime.world.cursor.get" || feature == "runtime.world.building.input"
            || feature == "runtime.world.voxel.read" {
            runtime_provider::world_operation_available(&feature)
        } else if feature == "runtime.world.voxel.grid_spec" {
            runtime_provider::world_operation_available("runtime.world.voxel.read")
        } else if feature == "runtime.world.entity.set_scale" {
            runtime_provider::world_entity_query_props_ready()
                && runtime_provider::can_write_component("keen::ecs::CurrentTransform")
        } else if feature == "runtime.world.entity.destroy" || feature == "runtime.world.entity.spawn" {
            runtime_provider::world_operation_available(&feature)
                && runtime_provider::world_entity_query_props_ready()
        } else if feature == "runtime.world.entity.query_props"
            || feature == "runtime.world.entity.query_props_in_bounds"
            || feature == "runtime.world.entity.register_prop_recipes"
            || feature == "runtime.world.entity.get_transform"
        {
            runtime_provider::world_entity_query_props_ready()
        } else if feature.starts_with("runtime.world.entity.") {
            runtime_provider::world_operation_available(&feature)
                && runtime_provider::world_entity_context_ready()
        } else if feature.starts_with("runtime.world.") {
            runtime_provider::world_operation_available(&feature)
                && runtime_provider::world_context_active()
        } else {
            true
        };
    if feature.starts_with("runtime.world.")
        && matches!(availability, Availability::Available)
        && !world_ready
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: format!("KFC Runtime world operation is not ready: {feature}"),
            },
        );
    }
    availability_to_lua(lua, availability)
}

pub(super) fn lua_require(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<()> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let feature = args.get::<String>(0)?;
    if available(&state, r#mod, &feature) {
        Ok(())
    } else {
        Err(LuaError::generic(format!(
            "ShroudForge feature is unavailable: {feature}"
        )))
    }
}

pub(super) fn lua_report_effect(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if state.phase() != RuntimePhase::Ingame || !has_capability(r#mod, Capability::Runtime) {
        return Ok((
            false,
            Some("runtime effect reporting requires an active ingame runtime mod".into()),
        ));
    }
    if !state.runtime_mod_is_active(&r#mod.info().id) {
        return Ok((false, Some("runtime mod is not active".into())));
    }
    let status = args.get::<String>(0)?;
    if ![
        "waiting",
        "no-target",
        "no-change",
        "write-confirmed",
        "write-failed",
    ]
    .contains(&status.as_str())
    {
        return Err(LuaError::generic(
            "runtime.report_effect state must be waiting, no-target, no-change, write-confirmed, or write-failed",
        ));
    }
    let detail = args.get::<Option<String>>(1)?.unwrap_or_default();
    state.report_runtime_effect(&r#mod.info().id, &status, &detail);
    Ok((true, None))
}

pub(super) fn unavailable_runtime_result(
    lua: &mlua::Lua,
    state: &AppState,
    operation: &str,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let availability = state.api().runtime(operation);
    match availability {
        Availability::Available => Ok((LuaValue::Table(lua.create_table()?), None)),
        Availability::Unavailable { reason } => Ok((LuaValue::Nil, Some(reason))),
    }
}

pub(super) fn availability_to_lua(
    lua: &mlua::Lua,
    availability: Availability,
) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    match availability {
        Availability::Available => {
            table.raw_set("available", true)?;
        }
        Availability::Unavailable { reason } => {
            table.raw_set("available", false)?;
            table.raw_set("reason", reason)?;
        }
    }

    Ok(table)
}

pub(super) fn has_capability_for_feature(r#mod: &Mod, feature: &str) -> bool {
    match feature {
        "game.assets.write" => has_capability(r#mod, Capability::Patch),
        "export" => has_capability(r#mod, Capability::Export),
        value if value.starts_with("runtime.") => has_capability(r#mod, Capability::Runtime),
        _ => true,
    }
}

pub(crate) fn runtime_denial_reason(
    state: &AppState,
    r#mod: &Mod,
    feature: &str,
) -> Option<String> {
    if state.phase() != RuntimePhase::Ingame {
        return Some("runtime APIs are available only during the ingame phase".into());
    }
    if !has_capability(r#mod, Capability::Runtime) {
        return Some(format!(
            "mod '{}' requires capabilities: [\"runtime\"]",
            r#mod.info().id
        ));
    }
    if !state.runtime_mod_is_active(&r#mod.info().id) {
        return Some(
            "runtime API access is available only while this mod's lifecycle is active".into(),
        );
    }
    if matches!(feature, "runtime.ecs.write" | "runtime.assets.write") {
        if let Some(reason) = state.runtime_write_fault() {
            return Some(reason);
        }
    }
    match state.api().runtime(feature) {
        Availability::Available => None,
        Availability::Unavailable { reason } => Some(reason),
    }
}
