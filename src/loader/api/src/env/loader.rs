use kfc::reflection::{PrimitiveType, TypeMetadata, TypeRegistry};
use mod_loader::{Capability, Mod};
use shroudforge_compatibility::Availability;
use std::rc::Rc;

const KFC_RUNTIME_ABI_VERSION: u32 = 12;

use crate::{
    RuntimePhase,
    alias::MappedValue,
    env::{
        AppFeatures, AppState,
        game::value::{
            convert_lua_to_value, convert_value_to_lua, mapped_source_bytes,
            validate_and_clone_lua_value,
        },
        util::add_function_with_mod,
    },
    lua::{FunctionArgs, LuaError, LuaValue},
};

fn is_component_type(registry: &TypeRegistry, metadata: &TypeMetadata) -> bool {
    if !metadata.qualified_name.starts_with("keen::ecs::")
        || metadata.qualified_name == "keen::ecs::Component"
    {
        return false;
    }
    let mut current = Some(metadata);
    for _ in 0..=registry.len() {
        let Some(ty) = current else {
            return false;
        };
        if ty.qualified_name == "keen::ecs::Component" {
            return true;
        }
        current = registry.get_inner_type(ty);
    }
    false
}

fn is_runtime_component_type(registry: &TypeRegistry, metadata: &TypeMetadata) -> bool {
    if !metadata.qualified_name.starts_with("keen::ecs::")
        || metadata.qualified_name == "keen::ecs::Component"
    {
        return false;
    }
    // Dynamic ECS columns are serialized as flat runtime structs instead of
    // deriving from Component. The provider still requires a verified
    // build-profile mapping or a validated live registration before resolving one.
    metadata.qualified_name.starts_with("keen::ecs::Dynamic")
        || is_component_type(registry, metadata)
}

pub(crate) fn runtime_provider_report() -> serde_json::Value {
    runtime_provider::report()
}

/// Creates the canonical `runtime.*` namespace used in every execution phase.
pub fn create(lua: &mlua::Lua, r#mod: Mod) -> mlua::Result<mlua::Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();
    if app_state.phase() == RuntimePhase::Ingame && !app_state.runtime_configured.get() {
        let registry = app_state.type_registry();
        let ecs_catalog_types = registry
            .iter()
            .filter(|metadata| metadata.qualified_name.starts_with("keen::ecs::"))
            .count();
        let component_candidates = registry
            .iter()
            .filter(|metadata| metadata.size > 0 && is_component_type(registry, metadata))
            .count();
        let dynamic_candidates = registry
            .iter()
            .filter(|metadata| {
                metadata.size > 0
                    && metadata.qualified_name.starts_with("keen::ecs::Dynamic")
                    && !is_component_type(registry, metadata)
            })
            .count();
        let contract: Vec<_> = registry
            .iter()
            .filter(|metadata| {
                metadata.size > 0 && metadata.qualified_name.starts_with("keen::ecs::")
            })
            .map(|metadata| (metadata.qualified_name.clone(), metadata.size))
            .collect();
        tracing::debug!(
            target: "shroudforge::runtime",
            reflection_types = registry.len(),
            ecs_catalog_types,
            component_candidates,
            dynamic_runtime_struct_candidates = dynamic_candidates,
            index_candidates = contract.len(),
            "Prepared KFC runtime component index candidates"
        );
        let configured = runtime_provider::configure(&contract);
        if configured {
            app_state.runtime_configured.set(true);
        }
        if !configured {
            tracing::error!("KFC Runtime rejected the component contract");
        }
        tracing::debug!(
            target: "shroudforge::runtime",
            configured,
            components = contract.len(),
            "KFC runtime component contract configuration finished"
        );
    }
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

    table.raw_set("ecs", create_ecs(lua, &r#mod)?)?;
    table.raw_set("world", create_world(lua, &r#mod)?)?;
    table.raw_set(
        "network",
        crate::env::runtime_networking::create(lua, &r#mod)?,
    )?;
    table.raw_set("patch", create_patch(lua, &r#mod)?)?;
    table.raw_set("functions", create_functions(lua, &table, &r#mod)?)?;

    Ok(table)
}

fn create_functions(
    lua: &mlua::Lua,
    runtime: &mlua::Table,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let result = lua.create_table()?;
    add_function_with_mod(lua, &result, "get_operations", r#mod, lua_get_operations)?;
    crate::env::runtime_functions::attach(lua, &result, &runtime.raw_get::<mlua::Table>("patch")?)?;
    // Capture existing checked wrappers. No second calling convention/dispatcher.
    let bindings = lua.create_table()?;
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../shroudforge/v1/runtime-operations.json"))
            .map_err(LuaError::external)?;
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

fn has_capability(r#mod: &Mod, capability: Capability) -> bool {
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
        | "runtime.network.accept" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && shroudforge_steam_networking::status().available
        }
        "runtime.world.context.active" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_operation_available(feature)
                && runtime_provider::world_context_active()
        }
        "runtime.world.cursor.get" | "runtime.world.building.input" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::world_operation_available(feature)
        }
        "runtime.world.voxel.read" | "runtime.world.voxel.write" => {
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
        "runtime.world.entity.spawn"
        | "runtime.world.entity.query_props"
        | "runtime.world.entity.query_props_in_bounds"
        | "runtime.world.entity.register_prop_recipes"
        | "runtime.world.entity.get_transform"
        | "runtime.world.entity.set_scale"
        | "runtime.world.entity.place"
        | "runtime.world.entity.destroy"
        | "runtime.world.entity.finish_building" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && if feature == "runtime.world.entity.query_props"
                    || feature == "runtime.world.entity.query_props_in_bounds"
                    || feature == "runtime.world.entity.register_prop_recipes"
                    || feature == "runtime.world.entity.get_transform"
                    || feature == "runtime.world.entity.set_scale"
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

fn lua_has(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<bool> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    Ok(available(&state, r#mod, &args.get::<String>(0)?))
}

fn lua_status(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    let feature = args.get::<String>(0)?;
    operation_status(lua, &feature, r#mod)
}

fn lua_get_operations(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../shroudforge/v1/runtime-operations.json"))
            .map_err(LuaError::external)?;
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

fn operation_status(lua: &mlua::Lua, feature: &str, r#mod: &Mod) -> mlua::Result<mlua::Table> {
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
        && !shroudforge_steam_networking::status().available
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "Steam Networking Messages is not initialized in this process".into(),
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
    let world_ready =
        if feature == "runtime.world.cursor.get" || feature == "runtime.world.building.input" {
            runtime_provider::world_operation_available(&feature)
        } else if feature == "runtime.world.voxel.grid_spec" {
            runtime_provider::world_operation_available("runtime.world.voxel.read")
        } else if feature == "runtime.world.entity.query_props"
            || feature == "runtime.world.entity.query_props_in_bounds"
            || feature == "runtime.world.entity.register_prop_recipes"
            || feature == "runtime.world.entity.get_transform"
            || feature == "runtime.world.entity.set_scale"
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

fn lua_require(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<()> {
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

fn lua_report_effect(
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

fn create_ecs(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;

    add_function_with_mod(lua, &table, "get_components", r#mod, lua_ecs_get_components)?;
    add_function_with_mod(lua, &table, "get_catalog", r#mod, lua_ecs_get_catalog)?;
    add_function_with_mod(lua, &table, "get_registry", r#mod, lua_ecs_get_registry)?;
    add_function_with_mod(lua, &table, "get_component", r#mod, lua_ecs_get_component)?;
    add_function_with_mod(lua, &table, "query", r#mod, lua_ecs_query)?;
    add_function_with_mod(lua, &table, "query_bounds", r#mod, lua_ecs_query_bounds)?;
    add_function_with_mod(lua, &table, "resolve", r#mod, lua_ecs_resolve)?;
    add_function_with_mod(lua, &table, "read", r#mod, lua_ecs_read)?;
    add_function_with_mod(lua, &table, "read_bytes", r#mod, lua_ecs_read_bytes)?;
    add_function_with_mod(lua, &table, "write", r#mod, lua_ecs_write)?;
    crate::env::runtime_attributes::attach(lua, &table, r#mod)?;

    Ok(table)
}

fn create_world(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    let world = lua.create_table()?;
    add_function_with_mod(
        lua,
        &world,
        "operation_available",
        r#mod,
        lua_world_operation_available,
    )?;
    add_function_with_mod(
        lua,
        &world,
        "context_active",
        r#mod,
        lua_world_context_active,
    )?;
    add_function_with_mod(lua, &world, "session_id", r#mod, lua_world_session_id)?;
    let voxel = lua.create_table()?;
    add_function_with_mod(lua, &voxel, "read", r#mod, lua_world_voxel_read)?;
    add_function_with_mod(lua, &voxel, "write", r#mod, lua_world_voxel_write)?;
    add_function_with_mod(lua, &voxel, "get_grid_spec", r#mod, lua_world_grid_get_spec)?;
    world.raw_set("voxel", voxel)?;
    let cursor = lua.create_table()?;
    add_function_with_mod(lua, &cursor, "get", r#mod, lua_world_cursor_get)?;
    world.raw_set("cursor", cursor)?;
    let entity = lua.create_table()?;
    add_function_with_mod(
        lua,
        &entity,
        "query_props",
        r#mod,
        lua_world_entity_query_props,
    )?;
    add_function_with_mod(
        lua,
        &entity,
        "query_props_in_bounds",
        r#mod,
        lua_world_entity_query_props_in_bounds,
    )?;
    add_function_with_mod(
        lua,
        &entity,
        "register_prop_recipes",
        r#mod,
        lua_world_entity_register_prop_recipes,
    )?;
    add_function_with_mod(
        lua,
        &entity,
        "get_transform",
        r#mod,
        lua_world_entity_get_transform,
    )?;
    add_function_with_mod(lua, &entity, "set_scale", r#mod, lua_world_entity_set_scale)?;
    add_function_with_mod(lua, &entity, "spawn", r#mod, lua_world_entity_spawn)?;
    add_function_with_mod(lua, &entity, "place", r#mod, lua_world_entity_place)?;
    add_function_with_mod(lua, &entity, "destroy", r#mod, lua_world_entity_destroy)?;
    add_function_with_mod(
        lua,
        &entity,
        "finish_building",
        r#mod,
        lua_world_entity_finish_building,
    )?;
    world.raw_set("entity", entity)?;
    crate::env::runtime_building::attach(lua, &world, r#mod)?;
    Ok(world)
}

fn create_patch(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    let patch = lua.create_table()?;
    add_function_with_mod(lua, &patch, "available", r#mod, lua_runtime_patch_available)?;
    add_function_with_mod(
        lua,
        &patch,
        "set_enabled",
        r#mod,
        lua_runtime_patch_set_enabled,
    )?;
    Ok(patch)
}

fn lua_runtime_patch_available(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<bool> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let name = args.get::<String>(0)?;
    if runtime_denial_reason(&state, r#mod, "runtime.gameplay.patch").is_some() {
        return Ok(false);
    }
    Ok(runtime_provider::runtime_patch_available(&name))
}

fn lua_runtime_patch_set_enabled(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.gameplay.patch") {
        return Ok((false, Some(reason)));
    }
    if args.len() != 2 {
        return Err(LuaError::generic(
            "runtime.patch.set_enabled expects profile patch name and enabled:boolean",
        ));
    }
    let name = args.get::<String>(0)?;
    let enabled = args.get::<bool>(1)?;
    if !runtime_provider::runtime_patch_available(&name) {
        return Ok((
            false,
            Some(format!("build profile does not resolve {name}")),
        ));
    }
    match runtime_provider::runtime_patch_set_enabled(&name, enabled) {
        Ok(()) => Ok((true, None)),
        Err(reason) => Ok((false, Some(reason))),
    }
}

fn lua_world_entity_spawn(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.spawn") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 6 {
        return Err(LuaError::generic(
            "runtime.world.entity.spawn expects templateUuidHighHex, templateUuidLowHex, position[3], rotation[4], trackingId, flags",
        ));
    }
    let high =
        u64::from_str_radix(args.get::<String>(0)?.trim_start_matches("0x"), 16).map_err(|_| {
            LuaError::generic("template UUID high must be a 16-digit hexadecimal string")
        })?;
    let low =
        u64::from_str_radix(args.get::<String>(1)?.trim_start_matches("0x"), 16).map_err(|_| {
            LuaError::generic("template UUID low must be a 16-digit hexadecimal string")
        })?;
    let position = lua_vec::<3>(args.get::<mlua::Table>(2)?.clone(), "position")?;
    let rotation = lua_vec::<4>(args.get::<mlua::Table>(3)?.clone(), "rotation")?;
    let tracking = args.get::<u32>(4)?;
    let flags = args.get::<u32>(5)?;
    match runtime_provider::world_entity_spawn([high, low], position, rotation, tracking, flags) {
        Ok(token) => Ok((LuaValue::Integer(i64::from(token)), None)),
        Err(reason) => Ok((LuaValue::Nil, Some(reason))),
    }
}

fn lua_world_entity_query_props(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.query_props") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 2 {
        return Err(LuaError::generic(
            "runtime.world.entity.query_props expects bounds[6] and padding",
        ));
    }
    let bounds_table = args.get::<mlua::Table>(0)?;
    let mut bounds = [0.0_f64; 6];
    for (index, value) in bounds.iter_mut().enumerate() {
        *value = bounds_table.raw_get::<f64>(index + 1)?;
        if !value.is_finite() {
            return Ok((
                LuaValue::Nil,
                Some("bounds must contain six finite numbers".into()),
            ));
        }
    }
    let padding = args.get::<f64>(1)?;
    if !padding.is_finite() || padding < 0.0 {
        return Ok((
            LuaValue::Nil,
            Some("padding must be a finite non-negative number".into()),
        ));
    }
    let props = match runtime_provider::world_entity_query_props(bounds, padding) {
        Ok(props) => props,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason.into()))),
    };
    let result = lua.create_table_with_capacity(props.len(), 0)?;
    for (index, prop) in props.into_iter().enumerate() {
        let item = lua.create_table()?;
        item.raw_set("handle", prop.entity_handle)?;
        item.raw_set(
            "entityId",
            runtime_provider::world_entity_identity(prop.entity_handle).unwrap_or(prop.entity_id),
        )?;
        item.raw_set("itemId", prop.item_id)?;
        item.raw_set(
            "templateUuidHighHex",
            format!("{:016x}", prop.template_uuid[0]),
        )?;
        item.raw_set(
            "templateUuidLowHex",
            format!("{:016x}", prop.template_uuid[1]),
        )?;
        let position = lua.create_table()?;
        let position_names = ["x", "y", "z"];
        for axis in 0..3 {
            let value = prop.position[axis] as f64 / 4_294_967_296.0;
            position.raw_set(axis + 1, value)?;
            position.raw_set(position_names[axis], value)?;
        }
        let orientation = lua.create_table()?;
        let orientation_names = ["x", "y", "z", "w"];
        for axis in 0..4 {
            orientation.raw_set(axis + 1, prop.orientation[axis])?;
            orientation.raw_set(orientation_names[axis], prop.orientation[axis])?;
        }
        let scale = lua.create_table()?;
        for axis in 0..3 {
            scale.raw_set(axis + 1, prop.scale[axis])?;
            scale.raw_set(position_names[axis], prop.scale[axis])?;
        }
        let transform = lua.create_table()?;
        transform.raw_set("position", position)?;
        transform.raw_set("orientation", orientation)?;
        transform.raw_set("scale", scale)?;
        item.raw_set("transform", transform)?;
        result.raw_set(index + 1, item)?;
    }
    Ok((LuaValue::Table(result), None))
}

fn lua_world_entity_register_prop_recipes(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) =
        runtime_denial_reason(&state, r#mod, "runtime.world.entity.register_prop_recipes")
    {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 1 {
        return Err(LuaError::generic(
            "runtime.world.entity.register_prop_recipes expects a recipe array",
        ));
    }
    let input = args.get::<mlua::Table>(0)?;
    let count = input.raw_len();
    if count > 1_000_000 {
        return Ok((
            LuaValue::Nil,
            Some("prop recipe catalog exceeds the 1,000,000-entry safety limit".into()),
        ));
    }
    let mut recipes = Vec::with_capacity(count);
    for index in 1..=count {
        let item = input.raw_get::<mlua::Table>(index)?;
        let item_id = item.raw_get::<u32>("itemId")?;
        let bounds = lua_vec::<6>(item.raw_get::<mlua::Table>("bounds")?, "recipe bounds")?
            .map(|value| value as f32);
        if item_id == 0
            || bounds.iter().any(|value| !value.is_finite())
            || (0..3).any(|axis| bounds[axis] > bounds[axis + 3])
        {
            return Ok((
                LuaValue::Nil,
                Some(format!("invalid placement recipe at index {index}")),
            ));
        }
        let feedback = item.raw_get::<u32>("feedback")?;
        recipes.push(runtime_provider::PropRecipe {
            item_id,
            bounds,
            feedback,
        });
    }
    match runtime_provider::world_entity_register_prop_recipes(&recipes) {
        Ok(()) => Ok((LuaValue::Boolean(true), None)),
        Err(reason) => Ok((LuaValue::Nil, Some(reason))),
    }
}

fn lua_world_entity_query_props_in_bounds(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let query_started = std::time::Instant::now();
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) =
        runtime_denial_reason(&state, r#mod, "runtime.world.entity.query_props_in_bounds")
    {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 1 {
        return Err(LuaError::generic(
            "runtime.world.entity.query_props_in_bounds expects bounds[6]",
        ));
    }
    let bounds = lua_vec::<6>(args.get::<mlua::Table>(0)?.clone(), "bounds")?;
    if bounds.iter().any(|value| !value.is_finite())
        || (0..3).any(|axis| bounds[axis] >= bounds[axis + 3])
    {
        return Ok((
            LuaValue::Nil,
            Some("bounds must contain finite minimum xyz and maximum xyz values".into()),
        ));
    }
    let props = match runtime_provider::world_entity_query_props_in_bounds(bounds) {
        Ok(props) => props,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason.into()))),
    };
    tracing::info!(
        target: "shroudforge::runtime",
        elapsed_ms = query_started.elapsed().as_millis() as u64,
        matched_props = props.len(),
        "Native recipe-bounds prop query completed"
    );
    let result = lua.create_table_with_capacity(props.len(), 0)?;
    for (index, prop) in props.into_iter().enumerate() {
        let item = lua.create_table()?;
        item.raw_set("handle", prop.entity_handle)?;
        item.raw_set(
            "entityId",
            runtime_provider::world_entity_identity(prop.entity_handle).unwrap_or(prop.entity_id),
        )?;
        item.raw_set("itemId", prop.item_id)?;
        item.raw_set(
            "templateUuidHighHex",
            format!("{:016x}", prop.template_uuid[0]),
        )?;
        item.raw_set(
            "templateUuidLowHex",
            format!("{:016x}", prop.template_uuid[1]),
        )?;
        let position = lua.create_table()?;
        for axis in 0..3 {
            let value = prop.position[axis] as f64 / 4_294_967_296.0;
            position.raw_set(axis + 1, value)?;
            position.raw_set(["x", "y", "z"][axis], value)?;
        }
        let orientation = lua.create_table()?;
        for axis in 0..4 {
            orientation.raw_set(axis + 1, prop.orientation[axis])?;
            orientation.raw_set(["x", "y", "z", "w"][axis], prop.orientation[axis])?;
        }
        let scale = lua.create_table()?;
        for axis in 0..3 {
            scale.raw_set(axis + 1, prop.scale[axis])?;
            scale.raw_set(["x", "y", "z"][axis], prop.scale[axis])?;
        }
        let transform = lua.create_table()?;
        transform.raw_set("position", position)?;
        transform.raw_set("orientation", orientation)?;
        transform.raw_set("scale", scale)?;
        item.raw_set("transform", transform)?;
        result.raw_set(index + 1, item)?;
    }
    Ok((LuaValue::Table(result), None))
}

fn lua_world_entity_get_transform(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.get_transform")
    {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 1 {
        return Err(LuaError::generic(
            "runtime.world.entity.get_transform expects an entity handle",
        ));
    }
    let handle = args.get::<u32>(0)?;
    let Some(prop) = runtime_provider::world_entity_get_transform(handle) else {
        return Ok((
            LuaValue::Nil,
            Some("entity handle is stale or not a live prop".into()),
        ));
    };
    let item = lua.create_table()?;
    item.raw_set("handle", prop.entity_handle)?;
    item.raw_set(
        "entityId",
        runtime_provider::world_entity_identity(prop.entity_handle).unwrap_or(prop.entity_id),
    )?;
    item.raw_set("itemId", prop.item_id)?;
    item.raw_set(
        "templateUuidHighHex",
        format!("{:016x}", prop.template_uuid[0]),
    )?;
    item.raw_set(
        "templateUuidLowHex",
        format!("{:016x}", prop.template_uuid[1]),
    )?;
    let position = lua.create_table()?;
    let axes = ["x", "y", "z"];
    for axis in 0..3 {
        let value = prop.position[axis] as f64 / 4_294_967_296.0;
        position.raw_set(axis + 1, value)?;
        position.raw_set(axes[axis], value)?;
    }
    let orientation = lua.create_table()?;
    for (axis, name) in ["x", "y", "z", "w"].iter().enumerate() {
        orientation.raw_set(axis + 1, prop.orientation[axis])?;
        orientation.raw_set(*name, prop.orientation[axis])?;
    }
    let scale = lua.create_table()?;
    for axis in 0..3 {
        scale.raw_set(axis + 1, prop.scale[axis])?;
        scale.raw_set(axes[axis], prop.scale[axis])?;
    }
    let transform = lua.create_table()?;
    transform.raw_set("position", position)?;
    transform.raw_set("orientation", orientation)?;
    transform.raw_set("scale", scale)?;
    item.raw_set("transform", transform)?;
    Ok((LuaValue::Table(item), None))
}

fn lua_world_entity_set_scale(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.set_scale") {
        return Ok((false, Some(reason)));
    }
    if args.len() != 2 {
        return Err(LuaError::generic(
            "runtime.world.entity.set_scale expects an entity handle and scale[3]",
        ));
    }
    let handle = args.get::<u32>(0)?;
    let scale = lua_vec::<3>(args.get::<mlua::Table>(1)?.clone(), "scale")?;
    match runtime_provider::world_entity_set_scale(handle, scale) {
        Ok(()) => Ok((true, None)),
        Err(reason) => Ok((false, Some(reason))),
    }
}

fn lua_world_grid_get_spec(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.voxel.grid_spec") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 1 {
        return Err(LuaError::generic(
            "runtime.world.voxel.get_grid_spec expects a grid id, currently 'voxel'",
        ));
    }
    let id = args.get::<String>(0)?;
    match runtime_provider::world_grid_get_spec(&id) {
        Some(spec) => {
            let result = lua.create_table()?;
            result.raw_set("id", spec.id)?;
            for (field, values) in [("origin", spec.origin), ("cellSize", spec.cell_size)] {
                let value = lua.create_table()?;
                for axis in 0..3 {
                    value.raw_set(axis + 1, values[axis])?;
                    value.raw_set(["x", "y", "z"][axis], values[axis])?;
                }
                result.raw_set(field, value)?;
            }
            let maximum = lua.create_table()?;
            for axis in 0..3 {
                maximum.raw_set(axis + 1, spec.maximum[axis])?;
                maximum.raw_set(["x", "y", "z"][axis], spec.maximum[axis])?;
            }
            result.raw_set("maximum", maximum)?;
            Ok((LuaValue::Table(result), None))
        }
        None => Ok((
            LuaValue::Nil,
            Some("grid is unavailable; this backend currently exposes 'voxel'".into()),
        )),
    }
}

fn lua_world_entity_place(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    lua_world_entity_placement(lua, args, r#mod, false)
}

fn lua_world_entity_destroy(
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

fn lua_world_entity_placement(
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

fn lua_world_entity_finish_building(
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

fn lua_world_operation_available(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<bool> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let name = args.get::<String>(0)?;
    if runtime_denial_reason(&state, r#mod, &name).is_some() {
        return Ok(false);
    }
    Ok(runtime_provider::world_operation_available(&name))
}

fn lua_world_session_id(lua: &mlua::Lua, _args: FunctionArgs, r#mod: &Mod) -> mlua::Result<u64> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if runtime_denial_reason(&state, r#mod, "runtime.ecs.query").is_some() {
        return Ok(0);
    }
    Ok(runtime_provider::world_session_id())
}

fn lua_world_context_active(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.context.active") {
        return Ok((false, Some(reason)));
    }
    let active = runtime_provider::world_context_active();
    Ok((
        active,
        (!active).then(|| "active voxel world context is not available".into()),
    ))
}

fn lua_world_cursor_get(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.cursor.get") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 0 {
        return Err(LuaError::generic(
            "runtime.world.cursor.get does not accept arguments",
        ));
    }
    let Some((bytes, sequence)) = runtime_provider::world_cursor_read() else {
        return Ok((
            LuaValue::Nil,
            Some("native cursor hook has not published a live sample yet".into()),
        ));
    };
    if bytes.len() != 0xa0 || bytes[0x98] > 1 {
        return Ok((
            LuaValue::Nil,
            Some("native cursor snapshot failed layout validation".into()),
        ));
    }
    let transform = |base: usize| -> mlua::Result<mlua::Table> {
        let output = lua.create_table()?;
        let position = lua.create_table()?;
        for (axis, name) in ["x", "y", "z"].iter().enumerate() {
            let start = base + axis * 8;
            position.raw_set(
                *name,
                i64::from_le_bytes(bytes[start..start + 8].try_into().unwrap()),
            )?;
        }
        let rotation = lua.create_table()?;
        for index in 0..4 {
            let start = base + 0x18 + index * 4;
            rotation.raw_set(
                index + 1,
                f32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()),
            )?;
        }
        let scale = lua.create_table()?;
        for index in 0..3 {
            let start = base + 0x28 + index * 4;
            scale.raw_set(
                index + 1,
                f32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()),
            )?;
        }
        output.raw_set("position", position)?;
        output.raw_set("rotation", rotation)?;
        output.raw_set("scale", scale)?;
        Ok(output)
    };
    let value = lua.create_table()?;
    value.raw_set("primaryTransform", transform(0)?)?;
    value.raw_set("secondaryTransform", transform(0x38)?)?;
    value.raw_set("primaryFlags", bytes[0x70])?;
    value.raw_set("secondaryFlags", bytes[0x71])?;
    value.raw_set("material", bytes[0x72])?;
    value.raw_set(
        "selectionVersion",
        format!(
            "{}",
            u64::from_le_bytes(bytes[0x78..0x80].try_into().unwrap())
        ),
    )?;
    let selected = lua.create_table()?;
    selected.raw_set(
        "value",
        format!(
            "{:016x}",
            u64::from_le_bytes(bytes[0x80..0x88].try_into().unwrap())
        ),
    )?;
    selected.raw_set(
        "type",
        u32::from_le_bytes(bytes[0x88..0x8c].try_into().unwrap()),
    )?;
    value.raw_set("selectedObject", selected)?;
    value.raw_set("attachedToProp", bytes[0x98] != 0)?;
    let result = lua.create_table()?;
    result.raw_set("sequence", sequence)?;
    result.raw_set("value", value)?;
    Ok((LuaValue::Table(result), None))
}

fn lua_world_voxel_read(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.voxel.read") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 6 {
        return Err(LuaError::generic(
            "runtime.world.voxel.read expects x, y, z, sizeX, sizeY, sizeZ",
        ));
    }
    let origin = [
        args.get::<i32>(0)?,
        args.get::<i32>(1)?,
        args.get::<i32>(2)?,
    ];
    let dimensions = [
        args.get::<u32>(3)?,
        args.get::<u32>(4)?,
        args.get::<u32>(5)?,
    ];
    let count = dimensions
        .iter()
        .try_fold(1usize, |total, value| total.checked_mul(*value as usize));
    let Some(count) = count.filter(|count| *count > 0 && *count <= 65_536) else {
        return Ok((
            LuaValue::Nil,
            Some("voxel read must contain between 1 and 65,536 cells".into()),
        ));
    };
    let Some(values) = runtime_provider::world_voxel_read(origin, dimensions, count) else {
        return Ok((
            LuaValue::Nil,
            Some("voxel read failed, timed out, or active world context is unavailable".into()),
        ));
    };
    let result = lua.create_table_with_capacity(values.len(), 0)?;
    for (index, value) in values.into_iter().enumerate() {
        result.raw_set(index + 1, value)?;
    }
    Ok((LuaValue::Table(result), None))
}

fn lua_world_voxel_write(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.voxel.write") {
        return Ok((false, Some(reason)));
    }
    if args.len() != 7 {
        return Err(LuaError::generic(
            "runtime.world.voxel.write expects x, y, z, sizeX, sizeY, sizeZ, cells",
        ));
    }
    let origin = [
        args.get::<i32>(0)?,
        args.get::<i32>(1)?,
        args.get::<i32>(2)?,
    ];
    let dimensions = [
        args.get::<u32>(3)?,
        args.get::<u32>(4)?,
        args.get::<u32>(5)?,
    ];
    let values = args.get::<mlua::Table>(6)?;
    let count = dimensions
        .iter()
        .try_fold(1usize, |total, value| total.checked_mul(*value as usize));
    let Some(count) = count.filter(|count| *count > 0 && *count <= 65_536) else {
        return Ok((
            false,
            Some("voxel write must contain between 1 and 65,536 cells".into()),
        ));
    };
    if values.raw_len() != count {
        return Ok((
            false,
            Some(format!(
                "voxel cell count mismatch: expected {count}, got {}",
                values.raw_len()
            )),
        ));
    }
    let mut cells = Vec::with_capacity(count);
    for index in 1..=count {
        let value = values.raw_get::<u32>(index)?;
        if value > u16::MAX as u32 {
            return Ok((
                false,
                Some(format!(
                    "voxel cell {index} exceeds the supported 16-bit format"
                )),
            ));
        }
        cells.push(value as u16);
    }
    match runtime_provider::world_voxel_write(origin, dimensions, &cells) {
        Ok(()) => Ok((true, None)),
        Err(reason) => Ok((false, Some(reason))),
    }
}

fn lua_ecs_resolve(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.resolve") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    let entity_id = args.get::<u32>(0)?;
    match runtime_provider::resolve_entity(entity_id) {
        Some(handle) => Ok((LuaValue::Integer(i64::from(handle)), None)),
        None => Ok((
            LuaValue::Nil,
            Some(format!("live ECS entity unavailable: {entity_id}")),
        )),
    }
}

fn lua_ecs_get_components(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.query") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if !runtime_provider::ready() {
        return Ok((LuaValue::Nil, Some("KFC Runtime is not ready".into())));
    }
    let result = lua.create_table()?;
    for metadata in state.type_registry().iter() {
        if !metadata.qualified_name.starts_with("keen::ecs::") {
            continue;
        }
        let Some(component) = runtime_provider::resolve(&metadata.qualified_name) else {
            continue;
        };
        if component.size != metadata.size {
            continue;
        }
        if let Some(value) = state.get_type(lua, metadata.index)? {
            result.push(value)?;
        }
    }
    Ok((LuaValue::Table(result), None))
}

/// Expose unresolved reflected candidates as well as the provider's usable mappings.
/// Being reflected alone never makes a component readable or writable.
fn lua_ecs_get_catalog(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let registry = state.type_registry();
    let result = lua.create_table()?;
    result.raw_set("version", registry.version.clone())?;
    let entries = lua.create_table()?;
    let ready = runtime_provider::ready();
    let can_write = runtime_provider::can_write();
    let read_denial = runtime_denial_reason(&state, r#mod, "runtime.ecs.read");
    let write_denial = runtime_denial_reason(&state, r#mod, "runtime.ecs.write");
    for metadata in registry
        .iter()
        .filter(|ty| is_runtime_component_type(registry, ty))
    {
        let entry = lua.create_table()?;
        entry.raw_set("type", state.get_type(lua, metadata.index)?)?;
        entry.raw_set("qualified_name", metadata.qualified_name.clone())?;
        entry.raw_set("qualified_hash", metadata.qualified_hash)?;
        entry.raw_set("size", metadata.size)?;
        let resolved = ready
            && runtime_provider::resolve(&metadata.qualified_name)
                .is_some_and(|component| component.size == metadata.size);
        entry.raw_set("resolved", resolved)?;
        let codec_reason =
            crate::env::runtime_values::validate_owned_type(registry, metadata.index).err();
        entry.raw_set("partial_value", codec_reason.is_some())?;
        entry.raw_set("value_reason", codec_reason.clone())?;
        entry.raw_set(
            "read_available",
            resolved
                && read_denial.is_none()
                && matches!(
                    state.api().runtime("runtime.ecs.read"),
                    Availability::Available
                ),
        )?;
        entry.raw_set(
            "write_available",
            resolved
                && codec_reason.is_none()
                && can_write
                && write_denial.is_none()
                && matches!(
                    state.api().runtime("runtime.ecs.write"),
                    Availability::Available
                ),
        )?;
        entry.raw_set("read_reason", read_denial.clone())?;
        entry.raw_set(
            "write_reason",
            write_denial.clone().or(codec_reason).or_else(|| {
                (!can_write).then(|| "runtime write dispatcher is not ready".to_owned())
            }),
        )?;
        entry.raw_set(
            "reason",
            if !ready {
                Some("KFC Runtime is not ready")
            } else if !resolved {
                Some("no runtime mapping with matching reflected size")
            } else {
                None
            },
        )?;
        entries.push(entry)?;
    }
    result.raw_set("entries", entries)?;
    Ok(result)
}

fn ecs_registry_entry(
    lua: &mlua::Lua,
    row: &serde_json::Value,
    r#mod: &Mod,
) -> mlua::Result<mlua::Table> {
    let LuaValue::Table(entry) = crate::env::registry::json_to_lua(lua, row)? else {
        unreachable!()
    };
    let state = lua.app_data_ref::<AppState>().unwrap();
    let mut codec_reason = None;
    for key in ["runtime_type", "template_type"] {
        if let Some(name) = row[key]["qualified_name"].as_str() {
            let ty = state
                .type_registry()
                .get_by_name(kfc::reflection::LookupKey::Qualified(name))
                .filter(|ty| {
                    ty.qualified_name == name && row[key]["size"].as_u64() == Some(ty.size as u64)
                })
                .ok_or_else(|| {
                    LuaError::generic("engine component registry and parser disagree")
                })?;
            entry.raw_set(key, state.get_type(lua, ty.index)?)?;
            if key == "runtime_type" {
                codec_reason = crate::env::runtime_values::validate_owned_type(
                    state.type_registry(),
                    ty.index,
                )
                .err();
            }
        }
    }
    let has_storage = !row["runtime_type"].is_null();
    for (key, operation) in [
        ("read_available", "runtime.ecs.read"),
        ("write_available", "runtime.ecs.write"),
    ] {
        entry.raw_set(
            key,
            has_storage
                && (operation != "runtime.ecs.write" || codec_reason.is_none())
                && available(&state, r#mod, operation)
                && runtime_denial_reason(&state, r#mod, operation).is_none(),
        )?;
    }
    entry.raw_set("partial_value", codec_reason.is_some())?;
    entry.raw_set("value_reason", codec_reason.clone())?;
    entry.raw_set("write_reason", codec_reason)?;
    if !has_storage {
        entry.raw_set(
            "reason",
            "template-only registration has no entity storage column",
        )?;
    }
    Ok(entry)
}

fn lua_ecs_get_registry(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let report = match runtime_provider::registry() {
        Ok(report) => report,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    if report["available"] != true {
        return Ok((
            LuaValue::Nil,
            Some(
                report["reason"]
                    .as_str()
                    .unwrap_or("component registry unavailable")
                    .into(),
            ),
        ));
    }
    let result = lua.create_table()?;
    for key in [
        "schema_version",
        "layout_version",
        "source",
        "count",
        "runtime_type_count",
    ] {
        result.raw_set(key, crate::env::registry::json_to_lua(lua, &report[key])?)?;
    }
    let entries = lua.create_table()?;
    for row in report["entries"]
        .as_array()
        .ok_or_else(|| LuaError::generic("invalid provider registry"))?
    {
        entries.push(ecs_registry_entry(lua, row, r#mod)?)?;
    }
    result.raw_set("entries", entries)?;
    Ok((LuaValue::Table(result), None))
}

fn lua_ecs_get_component(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let index = match crate::env::registry::resolve(state.type_registry(), args.get::<LuaValue>(0)?)
    {
        Ok(index) => index,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    let name = &state.type_registry().get(index).unwrap().qualified_name;
    let report = match runtime_provider::registry() {
        Ok(report) => report,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    for row in report["entries"].as_array().into_iter().flatten() {
        if row["qualified_name"].as_str() == Some(name)
            || row["runtime_type"]["qualified_name"].as_str() == Some(name)
        {
            return Ok((LuaValue::Table(ecs_registry_entry(lua, row, r#mod)?), None));
        }
    }
    Ok((
        LuaValue::Nil,
        Some(format!("no engine component registration for {name}")),
    ))
}

fn lua_ecs_query(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.query") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() == 0 {
        return Err(LuaError::generic(
            "runtime.ecs.query requires at least one keen::ecs type",
        ));
    }

    if !runtime_provider::ready() {
        return Ok((LuaValue::Nil, Some("KFC Runtime is not ready".into())));
    }

    let mut components = Vec::with_capacity(args.len());
    for index in 0..args.len() {
        let name = runtime_type_name(lua, &args, index)?;
        if runtime_provider::resolve(&name).is_none() {
            return Ok((
                LuaValue::Nil,
                Some(format!("live ECS component unavailable: {name}")),
            ));
        }
        components.push(name);
    }
    let entities = match runtime_provider::query(&components) {
        Ok(entities) => entities,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason.into()))),
    };
    let result = lua.create_table_with_capacity(entities.len(), 0)?;
    for (index, entity) in entities.into_iter().enumerate() {
        result.raw_set(index + 1, entity)?;
    }
    Ok((LuaValue::Table(result), None))
}

fn lua_ecs_query_bounds(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.query") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() < 3 {
        return Err(LuaError::generic(
            "runtime.ecs.query_bounds requires bounds[6], padding, and at least one keen::ecs type",
        ));
    }
    let bounds_table = args.get::<mlua::Table>(0)?;
    let padding = args.get::<f64>(1)?;
    if !padding.is_finite() || padding < 0.0 {
        return Ok((
            LuaValue::Nil,
            Some("padding must be a finite non-negative number".into()),
        ));
    }
    let mut bounds = [0.0_f64; 6];
    for (index, value) in bounds.iter_mut().enumerate() {
        *value = bounds_table.raw_get::<f64>(index + 1)?;
        if !value.is_finite() {
            return Ok((
                LuaValue::Nil,
                Some("bounds must contain six finite numbers".into()),
            ));
        }
    }
    if !runtime_provider::ready() {
        return Ok((LuaValue::Nil, Some("KFC Runtime is not ready".into())));
    }
    let mut components = Vec::with_capacity(args.len() - 2);
    for index in 2..args.len() {
        let name = runtime_type_name(lua, &args, index)?;
        if runtime_provider::resolve(&name).is_none() {
            return Ok((
                LuaValue::Nil,
                Some(format!("live ECS component unavailable: {name}")),
            ));
        }
        components.push(name);
    }
    let entities = match runtime_provider::query_bounds(&components, bounds, padding) {
        Ok(entities) => entities,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason.into()))),
    };
    let result = lua.create_table_with_capacity(entities.len(), 0)?;
    for (index, entity) in entities.into_iter().enumerate() {
        result.raw_set(index + 1, entity)?;
    }
    Ok((LuaValue::Table(result), None))
}

fn lua_ecs_read(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    lua_ecs_read_impl(lua, args, r#mod, false)
}

fn lua_ecs_read_bytes(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    lua_ecs_read_impl(lua, args, r#mod, true)
}

fn lua_ecs_read_impl(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
    raw: bool,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.read") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if !matches!(
        state.api().runtime("runtime.ecs.read"),
        Availability::Available
    ) {
        return unavailable_runtime_result(lua, &state, "runtime.ecs.read");
    }
    let entity_id = args.get::<u32>(0)?;
    let component_name = runtime_type_name(lua, &args, 1)?;
    let Some(component) = runtime_provider::resolve(&component_name) else {
        return Ok((
            LuaValue::Nil,
            Some(format!("live ECS component unavailable: {component_name}")),
        ));
    };
    let Some(metadata) = state
        .type_registry()
        .get_by_name(kfc::reflection::LookupKey::Qualified(&component_name))
    else {
        return Ok((
            LuaValue::Nil,
            Some(format!("type metadata unavailable: {component_name}")),
        ));
    };
    if metadata.size != component.size {
        return Ok((
            LuaValue::Nil,
            Some(format!(
                "component layout mismatch for {component_name}: parser={}, runtime={}",
                metadata.size, component.size
            )),
        ));
    }
    let Some(bytes) = runtime_provider::read(entity_id, &component_name, component.size) else {
        return Ok((
            LuaValue::Nil,
            Some(format!("component read failed: {component_name}")),
        ));
    };
    if raw {
        return Ok((LuaValue::String(lua.create_string(&bytes)?), None));
    }
    let bytes: Rc<[u8]> = bytes.into();
    let mapped = MappedValue::from_bytes(state.type_registry(), metadata, &bytes)
        .map_err(LuaError::external)?;
    Ok((convert_value_to_lua(&mapped, lua)?, None))
}

fn lua_ecs_write(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.write") {
        return Ok((false, Some(reason)));
    }
    match state.api().runtime("runtime.ecs.write") {
        Availability::Unavailable { reason } => return Ok((false, Some(reason))),
        Availability::Available => {}
    }
    if !runtime_provider::ready() {
        return Ok((false, Some("KFC Runtime is not ready".into())));
    }
    let entity_id = args.get::<u32>(0)?;
    let component_name = runtime_type_name(lua, &args, 1)?;
    let lua_value = args.get::<LuaValue>(2)?;
    let Some(component) = runtime_provider::resolve(&component_name) else {
        return Ok((
            false,
            Some(format!("live ECS component unavailable: {component_name}")),
        ));
    };
    let registry = state.type_registry();
    let Some(metadata) =
        registry.get_by_name(kfc::reflection::LookupKey::Qualified(&component_name))
    else {
        return Ok((
            false,
            Some(format!("type metadata unavailable: {component_name}")),
        ));
    };
    if metadata.size != component.size {
        return Ok((
            false,
            Some(format!(
                "component layout mismatch for {component_name}: parser={}, runtime={}",
                metadata.size, component.size
            )),
        ));
    }
    if let Err(reason) = crate::env::runtime_values::validate_owned_type(registry, metadata.index) {
        return Ok((false, Some(reason)));
    }
    let Some(source) = mapped_source_bytes(&lua_value)? else {
        return Ok((
            false,
            Some(format!(
                "runtime.ecs.write requires the value returned by runtime.ecs.read for {component_name}"
            )),
        ));
    };
    if source.len() != component.size as usize {
        return Ok((
            false,
            Some(format!("runtime source size mismatch: {component_name}")),
        ));
    }
    let handle = crate::alias::TypeHandle::new(registry.clone(), metadata.index);
    let checked =
        validate_and_clone_lua_value(&lua_value, &handle, lua).map_err(LuaError::external)?;
    let value = convert_lua_to_value(&checked, &handle).map_err(LuaError::external)?;
    let bytes = value
        .to_bytes(registry, metadata)
        .map_err(LuaError::external)?;
    if bytes.len() != component.size as usize {
        return Ok((
            false,
            Some(format!(
                "serialized component size mismatch: {component_name}"
            )),
        ));
    }
    let mut ranges = Vec::new();
    collect_changed_ranges(registry, metadata, 0, &source, &bytes, &mut ranges)
        .map_err(LuaError::generic)?;
    if ranges.is_empty() {
        return Ok((true, None));
    }
    let mut mask = vec![0u8; component.size as usize];
    for (start, end) in ranges {
        mask[start..end].fill(1);
    }
    if runtime_provider::write(entity_id, &component_name, &mask, &bytes) {
        Ok((true, None))
    } else {
        Ok((
            false,
            Some(format!("component write failed: {component_name}")),
        ))
    }
}

fn collect_changed_ranges(
    registry: &TypeRegistry,
    metadata: &TypeMetadata,
    base: usize,
    source: &[u8],
    candidate: &[u8],
    ranges: &mut Vec<(usize, usize)>,
) -> Result<(), String> {
    let end = base
        .checked_add(metadata.size as usize)
        .ok_or_else(|| format!("runtime layout overflow: {}", metadata.qualified_name))?;
    if end > source.len() || end > candidate.len() {
        return Err(format!(
            "runtime layout outside component: {}",
            metadata.qualified_name
        ));
    }
    match metadata.primitive_type {
        PrimitiveType::Typedef => {
            let inner = registry
                .get_inner_type(metadata)
                .ok_or_else(|| format!("missing inner type: {}", metadata.qualified_name))?;
            collect_changed_ranges(registry, inner, base, source, candidate, ranges)
        }
        PrimitiveType::Struct => {
            if let Some(parent) = registry.get_inner_type(metadata) {
                collect_changed_ranges(registry, parent, base, source, candidate, ranges)?;
            }
            for field in metadata.struct_fields.values() {
                let field_type = registry.get(field.r#type).ok_or_else(|| {
                    format!(
                        "missing field type: {}.{}",
                        metadata.qualified_name, field.name
                    )
                })?;
                let offset = usize::try_from(field.data_offset).map_err(|_| {
                    format!(
                        "invalid field offset: {}.{}",
                        metadata.qualified_name, field.name
                    )
                })?;
                collect_changed_ranges(
                    registry,
                    field_type,
                    base + offset,
                    source,
                    candidate,
                    ranges,
                )?;
            }
            Ok(())
        }
        PrimitiveType::StaticArray => {
            let inner = registry.get_inner_type(metadata).ok_or_else(|| {
                format!("missing array element type: {}", metadata.qualified_name)
            })?;
            for index in 0..metadata.field_count as usize {
                collect_changed_ranges(
                    registry,
                    inner,
                    base + index * inner.size as usize,
                    source,
                    candidate,
                    ranges,
                )?;
            }
            Ok(())
        }
        PrimitiveType::DsArray
        | PrimitiveType::DsString
        | PrimitiveType::DsOptional
        | PrimitiveType::DsVariant
        | PrimitiveType::BlobArray
        | PrimitiveType::BlobString
        | PrimitiveType::BlobOptional
        | PrimitiveType::BlobVariant => {
            if source[base..end] != candidate[base..end] {
                return Err(format!(
                    "runtime write for dynamic field type is not supported: {}",
                    metadata.qualified_name
                ));
            }
            Ok(())
        }
        _ => {
            if source[base..end] != candidate[base..end] {
                if let Some((_, previous_end)) =
                    ranges.last_mut().filter(|(_, value)| *value == base)
                {
                    *previous_end = end;
                } else {
                    ranges.push((base, end));
                }
            }
            Ok(())
        }
    }
}

fn runtime_type_name(lua: &mlua::Lua, args: &FunctionArgs, index: usize) -> mlua::Result<String> {
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

fn unavailable_runtime_result(
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

fn availability_to_lua(lua: &mlua::Lua, availability: Availability) -> mlua::Result<mlua::Table> {
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

fn has_capability_for_feature(r#mod: &Mod, feature: &str) -> bool {
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
    match state.api().runtime(feature) {
        Availability::Available => None,
        Availability::Unavailable { reason } => Some(reason),
    }
}

#[cfg(windows)]
pub(super) mod runtime_provider {
    use super::KFC_RUNTIME_ABI_VERSION;
    use std::{
        ffi::{CString, c_char, c_void},
        sync::OnceLock,
    };

    #[derive(Clone, Copy)]
    pub struct Component {
        pub size: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct PropRecord {
        pub entity_handle: u32,
        pub item_id: u32,
        pub position: [i64; 3],
        pub orientation: [f32; 4],
        pub scale: [f32; 3],
        pub template_uuid: [u64; 2],
        pub entity_id: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct PropRecipe {
        pub item_id: u32,
        pub bounds: [f32; 6],
        pub feedback: u32,
    }

    type Ready = unsafe extern "C" fn() -> bool;
    type Configure = unsafe extern "C" fn(*const *const c_char, *const u32, usize) -> bool;
    type Describe = unsafe extern "C" fn(*const c_char, *mut u32) -> bool;
    type Registry = unsafe extern "C" fn(*mut c_char, usize) -> usize;
    type FunctionCode = unsafe extern "C" fn(u32, *mut u8, usize) -> usize;
    type ResolveType = unsafe extern "C" fn(*const c_char, *mut c_char, usize) -> bool;
    type Query = unsafe extern "C" fn(*const *const c_char, usize, *mut u32, usize) -> usize;
    type QueryBounds = unsafe extern "C" fn(
        *const *const c_char,
        usize,
        *const f64,
        f64,
        *mut u32,
        usize,
    ) -> usize;
    type ResolveEntity = unsafe extern "C" fn(u32) -> u32;
    type Read = unsafe extern "C" fn(u32, *const c_char, *mut c_void, usize) -> bool;
    type Write =
        unsafe extern "C" fn(u32, *const c_char, *const c_void, *const c_void, usize) -> bool;
    type CompareExchange = unsafe extern "C" fn(
        u32,
        *const c_char,
        *const c_void,
        *const c_void,
        *const c_void,
        usize,
    ) -> u32;
    type WorldOperationAvailable = unsafe extern "C" fn(*const c_char) -> bool;
    type WorldContextActive = unsafe extern "C" fn() -> bool;
    type WorldSessionId = unsafe extern "C" fn() -> u64;
    type WorldEntityContextReady = unsafe extern "C" fn() -> bool;
    type WorldEntityQueryProps =
        unsafe extern "C" fn(*const f64, f64, *mut PropRecord, usize) -> usize;
    type WorldEntityQueryPropsInBounds =
        unsafe extern "C" fn(*const f64, *mut PropRecord, usize) -> usize;
    type WorldEntityRegisterPropRecipes = unsafe extern "C" fn(*const PropRecipe, usize) -> bool;
    type WorldEntityGetTransform = unsafe extern "C" fn(u32, *mut PropRecord) -> bool;
    type WorldEntityIdentity = unsafe extern "C" fn(u32, *mut u32) -> bool;
    type WorldEntitySetScale = unsafe extern "C" fn(u32, *const f64) -> bool;
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct GridSpec {
        id: [c_char; 16],
        origin: [f64; 3],
        cell_size: [f64; 3],
        maximum: [u64; 3],
    }
    pub struct GridSpecResult {
        pub id: String,
        pub origin: [f64; 3],
        pub cell_size: [f64; 3],
        pub maximum: [u64; 3],
    }
    type WorldGridGetSpec = unsafe extern "C" fn(*const c_char, *mut GridSpec) -> bool;
    type WorldCursorRead = unsafe extern "C" fn(*mut u8, usize, *mut u64) -> bool;
    type WorldVoxelRead =
        unsafe extern "C" fn(*const i32, *const u32, *mut u16, usize, *mut usize) -> bool;
    type WorldVoxelWrite =
        unsafe extern "C" fn(*const i32, *const u32, *const u16, usize, *mut u32) -> bool;
    type WorldEntitySpawn = unsafe extern "C" fn(
        *const u64,
        *const f64,
        *const f64,
        u32,
        u32,
        *mut u32,
        *mut u32,
    ) -> bool;
    type WorldEntityPlacement =
        unsafe extern "C" fn(*const f64, *const f64, *const f32, u32, u32, *mut u32) -> bool;
    type WorldEntityDestroyHandle = unsafe extern "C" fn(u32, *mut u32) -> bool;
    type WorldEntityFinish = unsafe extern "C" fn(bool, *mut u32) -> bool;
    type RuntimePatchAvailable = unsafe extern "C" fn(*const c_char) -> bool;
    type RuntimePatchSetEnabled = unsafe extern "C" fn(*const c_char, bool, *mut u32) -> bool;
    struct Provider {
        configure: Configure,
        ready: Ready,
        prop_query_ready: Option<Ready>,
        can_write: Ready,
        describe: Describe,
        registry: Option<Registry>,
        functions: Option<Registry>,
        function_code: Option<FunctionCode>,
        resolve_type: Option<ResolveType>,
        query: Query,
        query_bounds: Option<QueryBounds>,
        resolve_entity: ResolveEntity,
        read: Read,
        write: Write,
        compare_exchange: Option<CompareExchange>,
        world_operation_available: WorldOperationAvailable,
        world_context_active: WorldContextActive,
        world_session_id: Option<WorldSessionId>,
        world_entity_context_ready: WorldEntityContextReady,
        world_entity_query_props: WorldEntityQueryProps,
        world_entity_query_props_in_bounds: WorldEntityQueryPropsInBounds,
        world_entity_register_prop_recipes: WorldEntityRegisterPropRecipes,
        world_entity_get_transform: WorldEntityGetTransform,
        world_entity_identity: WorldEntityIdentity,
        world_entity_set_scale: WorldEntitySetScale,
        world_cursor_read: Option<WorldCursorRead>,
        world_voxel_read: WorldVoxelRead,
        world_voxel_write: WorldVoxelWrite,
        world_grid_get_spec: WorldGridGetSpec,
        world_entity_spawn: WorldEntitySpawn,
        world_entity_place: WorldEntityPlacement,
        world_entity_destroy: WorldEntityPlacement,
        world_entity_destroy_handle: WorldEntityDestroyHandle,
        world_entity_finish: WorldEntityFinish,
        runtime_patch_available: RuntimePatchAvailable,
        runtime_patch_set_enabled: RuntimePatchSetEnabled,
        abi: u32,
        status: unsafe extern "C" fn(*mut c_char, usize),
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *const c_void;
    }

    static PROVIDER: OnceLock<Provider> = OnceLock::new();
    static PROVIDER_ERROR: OnceLock<String> = OnceLock::new();

    fn provider() -> Option<&'static Provider> {
        if let Some(provider) = PROVIDER.get() {
            return Some(provider);
        }
        let loaded = unsafe {
            (|| -> Result<Provider, String> {
                let module_name: Vec<u16> = "kfc-runtime.dll\0".encode_utf16().collect();
                let module = GetModuleHandleW(module_name.as_ptr());
                if module.is_null() {
                    Err("provider-module-not-loaded".to_string())
                } else {
                    macro_rules! symbol {
                        ($name:literal, $kind:ty) => {{
                            let pointer =
                                GetProcAddress(module, concat!($name, "\0").as_ptr().cast());
                            if pointer.is_null() {
                                return Err(concat!("missing-export:", $name).to_string());
                            }
                            std::mem::transmute::<*const c_void, $kind>(pointer)
                        }};
                    }
                    let abi = symbol!("KfcRuntimeAbi", unsafe extern "C" fn() -> u32);
                    let actual_abi = abi();
                    if actual_abi != KFC_RUNTIME_ABI_VERSION {
                        Err(format!(
                            "provider-abi-mismatch:expected={KFC_RUNTIME_ABI_VERSION},actual={actual_abi}"
                        ))
                    } else {
                        Ok(Provider {
                            configure: symbol!("KfcRuntimeEcsConfigure", Configure),
                            ready: symbol!("KfcRuntimeEcsReady", Ready),
                            prop_query_ready: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeEcsPropQueryReady".as_ptr());
                                (!pointer.is_null())
                                    .then(|| std::mem::transmute::<*const c_void, Ready>(pointer))
                            },
                            can_write: symbol!("KfcRuntimeEcsCanWrite", Ready),
                            describe: symbol!("KfcRuntimeEcsDescribe", Describe),
                            registry: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeEcsRegistry".as_ptr());
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, Registry>(pointer)
                                })
                            },
                            functions: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeFunctions".as_ptr());
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, Registry>(pointer)
                                })
                            },
                            function_code: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeFunctionCode".as_ptr());
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, FunctionCode>(pointer)
                                })
                            },
                            resolve_type: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeEcsResolveType".as_ptr());
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, ResolveType>(pointer)
                                })
                            },
                            query: symbol!("KfcRuntimeEcsQuery", Query),
                            query_bounds: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeEcsQueryBounds".as_ptr());
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, QueryBounds>(pointer)
                                })
                            },
                            resolve_entity: symbol!("KfcRuntimeEcsResolve", ResolveEntity),
                            read: symbol!("KfcRuntimeEcsRead", Read),
                            write: symbol!("KfcRuntimeEcsWrite", Write),
                            compare_exchange: {
                                let pointer = GetProcAddress(
                                    module,
                                    c"KfcRuntimeEcsCompareExchange".as_ptr(),
                                );
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, CompareExchange>(pointer)
                                })
                            },
                            world_operation_available: symbol!(
                                "KfcRuntimeWorldOperationAvailable",
                                WorldOperationAvailable
                            ),
                            world_session_id: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeWorldSessionId".as_ptr());
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, WorldSessionId>(pointer)
                                })
                            },
                            world_context_active: symbol!(
                                "KfcRuntimeWorldContextActive",
                                WorldContextActive
                            ),
                            world_entity_context_ready: symbol!(
                                "KfcRuntimeWorldEntityContextReady",
                                WorldEntityContextReady
                            ),
                            world_entity_query_props: symbol!(
                                "KfcRuntimeWorldEntityQueryProps",
                                WorldEntityQueryProps
                            ),
                            world_entity_query_props_in_bounds: symbol!(
                                "KfcRuntimeWorldEntityQueryPropsInBounds",
                                WorldEntityQueryPropsInBounds
                            ),
                            world_entity_register_prop_recipes: symbol!(
                                "KfcRuntimeWorldEntityRegisterPropRecipes",
                                WorldEntityRegisterPropRecipes
                            ),
                            world_entity_get_transform: symbol!(
                                "KfcRuntimeWorldEntityGetTransform",
                                WorldEntityGetTransform
                            ),
                            world_entity_identity: symbol!(
                                "KfcRuntimeEcsEntityIdentity",
                                WorldEntityIdentity
                            ),
                            world_entity_set_scale: symbol!(
                                "KfcRuntimeWorldEntitySetScale",
                                WorldEntitySetScale
                            ),
                            world_cursor_read: {
                                let pointer =
                                    GetProcAddress(module, c"KfcRuntimeWorldCursorRead".as_ptr());
                                (!pointer.is_null()).then(|| {
                                    std::mem::transmute::<*const c_void, WorldCursorRead>(pointer)
                                })
                            },
                            world_voxel_read: symbol!("KfcRuntimeWorldVoxelRead", WorldVoxelRead),
                            world_voxel_write: symbol!(
                                "KfcRuntimeWorldVoxelWrite",
                                WorldVoxelWrite
                            ),
                            world_grid_get_spec: symbol!(
                                "KfcRuntimeWorldGridGetSpec",
                                WorldGridGetSpec
                            ),
                            world_entity_spawn: symbol!(
                                "KfcRuntimeWorldEntitySpawn",
                                WorldEntitySpawn
                            ),
                            world_entity_place: symbol!(
                                "KfcRuntimeWorldEntityPlace",
                                WorldEntityPlacement
                            ),
                            world_entity_destroy: symbol!(
                                "KfcRuntimeWorldEntityDestroy",
                                WorldEntityPlacement
                            ),
                            world_entity_destroy_handle: symbol!(
                                "KfcRuntimeWorldEntityDestroyHandle",
                                WorldEntityDestroyHandle
                            ),
                            world_entity_finish: symbol!(
                                "KfcRuntimeWorldEntityFinishBuilding",
                                WorldEntityFinish
                            ),
                            runtime_patch_available: symbol!(
                                "KfcRuntimePatchAvailable",
                                RuntimePatchAvailable
                            ),
                            runtime_patch_set_enabled: symbol!(
                                "KfcRuntimePatchSetEnabled",
                                RuntimePatchSetEnabled
                            ),
                            abi: actual_abi,
                            status: symbol!(
                                "KfcRuntimeStatus",
                                unsafe extern "C" fn(*mut c_char, usize)
                            ),
                        })
                    }
                }
            })()
        };
        match loaded {
            Ok(provider) => {
                let _ = PROVIDER.set(provider);
                PROVIDER.get()
            }
            Err(error) => {
                // The runtime DLL may be loaded just after Lua environments are
                // created. Treat that as a transient startup condition and try
                // again on the next API call instead of poisoning this process.
                if error != "provider-module-not-loaded" {
                    let _ = PROVIDER_ERROR.set(error);
                }
                None
            }
        }
    }

    fn observed_abi() -> Option<u32> {
        unsafe {
            let module_name: Vec<u16> = "kfc-runtime.dll\0".encode_utf16().collect();
            let module = GetModuleHandleW(module_name.as_ptr());
            if module.is_null() {
                return None;
            }
            let pointer = GetProcAddress(module, c"KfcRuntimeAbi".as_ptr());
            if pointer.is_null() {
                return None;
            }
            let abi: unsafe extern "C" fn() -> u32 = std::mem::transmute(pointer);
            Some(abi())
        }
    }

    pub fn configure(types: &[(String, u32)]) -> bool {
        let Some(provider) = provider() else {
            return false;
        };
        let Ok(names) = types
            .iter()
            .map(|(name, _)| CString::new(name.as_str()))
            .collect::<Result<Vec<_>, _>>()
        else {
            return false;
        };
        let pointers: Vec<_> = names.iter().map(|name| name.as_ptr()).collect();
        let sizes: Vec<_> = types.iter().map(|(_, size)| *size).collect();
        unsafe { (provider.configure)(pointers.as_ptr(), sizes.as_ptr(), pointers.len()) }
    }
    pub fn ready() -> bool {
        provider().is_some_and(|value| unsafe { (value.ready)() })
    }
    pub fn world_entity_query_props_ready() -> bool {
        provider().is_some_and(|value| unsafe {
            value
                .prop_query_ready
                .map_or_else(|| (value.ready)(), |ready| ready())
        })
    }
    pub fn can_write() -> bool {
        provider().is_some_and(|value| unsafe { (value.can_write)() })
    }
    pub fn world_operation_available(name: &str) -> bool {
        let Some(provider) = provider() else {
            return false;
        };
        let Ok(name) = CString::new(name) else {
            return false;
        };
        unsafe { (provider.world_operation_available)(name.as_ptr()) }
    }
    pub fn world_session_id() -> u64 {
        provider()
            .and_then(|value| value.world_session_id)
            .map_or(0, |get| unsafe { get() })
    }
    pub fn world_context_active() -> bool {
        provider().is_some_and(|value| unsafe { (value.world_context_active)() })
    }
    pub fn world_entity_context_ready() -> bool {
        provider().is_some_and(|value| unsafe { (value.world_entity_context_ready)() })
    }
    pub fn world_cursor_read() -> Option<(Vec<u8>, u64)> {
        let provider = provider()?;
        let mut bytes = vec![0u8; 0xa0];
        let mut sequence = 0u64;
        let read = provider.world_cursor_read?;
        let ok = unsafe { read(bytes.as_mut_ptr(), bytes.len(), &mut sequence) };
        (ok && sequence != 0).then_some((bytes, sequence))
    }
    pub fn world_voxel_read(
        origin: [i32; 3],
        dimensions: [u32; 3],
        count: usize,
    ) -> Option<Vec<u16>> {
        let provider = provider()?;
        let mut values = vec![0u16; count];
        let mut actual = 0usize;
        let ok = unsafe {
            (provider.world_voxel_read)(
                origin.as_ptr(),
                dimensions.as_ptr(),
                values.as_mut_ptr(),
                count,
                &mut actual,
            )
        };
        (ok && actual == count).then_some(values)
    }
    pub fn world_voxel_write(
        origin: [i32; 3],
        dimensions: [u32; 3],
        values: &[u16],
    ) -> Result<(), String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        let mut outcome = 1u32;
        let ok = unsafe {
            (provider.world_voxel_write)(
                origin.as_ptr(),
                dimensions.as_ptr(),
                values.as_ptr(),
                values.len(),
                &mut outcome,
            )
        };
        if ok {
            return Ok(());
        }
        Err(match outcome {
            1 => "voxel write was rejected before changing the world".into(),
            2 => "voxel write failed; the previous voxel region was restored and read back".into(),
            _ => "voxel write outcome is uncertain; rollback could not be verified".into(),
        })
    }
    fn operation_error(operation: &str, outcome: u32) -> String {
        match outcome {
            30..=61 => {
                let flags = outcome - 30;
                format!(
                    "{operation} was not consumed (hooks_ready={}, actor_hook_entered={}, actor_saw_request={}, building_dispatch_entered={}, building_dispatch_saw_request={})",
                    flags & 16 != 0,
                    flags & 1 != 0,
                    flags & 2 != 0,
                    flags & 4 != 0,
                    flags & 8 != 0,
                )
            }
            1 => format!(
                "{operation} was rejected before dispatch (profile, arguments, or live context unavailable)"
            ),
            2 => format!("{operation} failed in the engine call"),
            3 => format!(
                "{operation} was queued, but its native game hook did not consume it within 3 seconds"
            ),
            4 => format!(
                "{operation} was dispatched, but the requested live ECS state change was not observed; the final world state is uncertain"
            ),
            _ => format!(
                "{operation} timed out; the game thread may still have consumed the request"
            ),
        }
    }
    pub fn world_entity_spawn(
        uuid: [u64; 2],
        position: [f64; 3],
        rotation: [f64; 4],
        tracking: u32,
        flags: u32,
    ) -> Result<u32, String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        let mut token = 0u32;
        let mut outcome = 1u32;
        let ok = unsafe {
            (provider.world_entity_spawn)(
                uuid.as_ptr(),
                position.as_ptr(),
                rotation.as_ptr(),
                tracking,
                flags,
                &mut token,
                &mut outcome,
            )
        };
        if ok {
            Ok(token)
        } else {
            Err(operation_error("runtime.world.entity.spawn", outcome))
        }
    }
    pub fn world_entity_query_props(
        bounds: [f64; 6],
        padding: f64,
    ) -> Result<Vec<PropRecord>, &'static str> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable");
        };
        let count = unsafe {
            (provider.world_entity_query_props)(bounds.as_ptr(), padding, std::ptr::null_mut(), 0)
        };
        if count == usize::MAX {
            return Err("native prop query failed");
        }
        if count > 1_000_000 {
            return Err("native prop query exceeded the 1,000,000-prop safety limit");
        }
        let mut props = vec![PropRecord::default(); count];
        if count == 0 {
            return Ok(props);
        }
        let actual = unsafe {
            (provider.world_entity_query_props)(
                bounds.as_ptr(),
                padding,
                props.as_mut_ptr(),
                props.len(),
            )
        };
        if actual == usize::MAX {
            return Err("native prop query failed while retrieving results");
        }
        if actual > 1_000_000 {
            return Err("native prop query exceeded the 1,000,000-prop safety limit");
        }
        if actual > props.len() {
            return Err("native prop query changed between count and retrieval");
        }
        props.truncate(actual);
        Ok(props)
    }
    pub fn world_entity_register_prop_recipes(recipes: &[PropRecipe]) -> Result<(), String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        let accepted = unsafe {
            (provider.world_entity_register_prop_recipes)(recipes.as_ptr(), recipes.len())
        };
        if accepted {
            Ok(())
        } else {
            Err("native prop recipe registration was rejected".into())
        }
    }
    pub fn world_entity_query_props_in_bounds(
        bounds: [f64; 6],
    ) -> Result<Vec<PropRecord>, &'static str> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable");
        };
        let count = unsafe {
            (provider.world_entity_query_props_in_bounds)(bounds.as_ptr(), std::ptr::null_mut(), 0)
        };
        if count == usize::MAX - 1 {
            return Err("live prop query is still scanning; retry on the next update");
        }
        if count == usize::MAX {
            return Err("native recipe-bounds prop query failed");
        }
        if count > 1_000_000 {
            return Err("native prop query exceeded the 1,000,000-prop safety limit");
        }
        let mut props = vec![PropRecord::default(); count];
        if count == 0 {
            return Ok(props);
        }
        let actual = unsafe {
            (provider.world_entity_query_props_in_bounds)(
                bounds.as_ptr(),
                props.as_mut_ptr(),
                props.len(),
            )
        };
        if actual == usize::MAX - 1 {
            return Err("live prop query is still scanning; retry on the next update");
        }
        if actual == usize::MAX {
            return Err("native recipe-bounds prop query failed while retrieving results");
        }
        if actual > props.len() {
            return Err("native prop query changed between count and retrieval");
        }
        props.truncate(actual);
        Ok(props)
    }
    pub fn world_entity_get_transform(handle: u32) -> Option<PropRecord> {
        let provider = provider()?;
        let mut prop = PropRecord::default();
        let ok = unsafe { (provider.world_entity_get_transform)(handle, &mut prop) };
        ok.then_some(prop)
    }
    pub fn world_entity_identity(handle: u32) -> Option<u32> {
        let provider = provider()?;
        let mut entity_id = 0;
        let ok = unsafe { (provider.world_entity_identity)(handle, &mut entity_id) };
        (ok && entity_id != 0).then_some(entity_id)
    }
    pub fn world_entity_set_scale(handle: u32, scale: [f64; 3]) -> Result<(), String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        if scale.iter().any(|value| !value.is_finite()) {
            return Err("entity scale must contain finite values".into());
        }
        let ok = unsafe { (provider.world_entity_set_scale)(handle, scale.as_ptr()) };
        if ok {
            Ok(())
        } else {
            Err("native entity scale update failed or timed out".into())
        }
    }
    pub fn world_grid_get_spec(id: &str) -> Option<GridSpecResult> {
        let provider = provider()?;
        let id = CString::new(id).ok()?;
        let mut spec = GridSpec::default();
        let ok = unsafe { (provider.world_grid_get_spec)(id.as_ptr(), &mut spec) };
        if !ok {
            return None;
        }
        let id_end = spec
            .id
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(spec.id.len());
        let id = String::from_utf8_lossy(unsafe {
            std::slice::from_raw_parts(spec.id.as_ptr().cast::<u8>(), id_end)
        })
        .into_owned();
        Some(GridSpecResult {
            id,
            origin: spec.origin,
            cell_size: spec.cell_size,
            maximum: spec.maximum,
        })
    }
    pub fn world_entity_placement(
        destroy: bool,
        position: [f64; 3],
        rotation: [f64; 4],
        bounds: [f32; 6],
        tracking: u32,
        feedback: u32,
    ) -> Result<(), String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        let mut outcome = 1u32;
        let function = if destroy {
            provider.world_entity_destroy
        } else {
            provider.world_entity_place
        };
        let ok = unsafe {
            function(
                position.as_ptr(),
                rotation.as_ptr(),
                bounds.as_ptr(),
                tracking,
                feedback,
                &mut outcome,
            )
        };
        if ok {
            Ok(())
        } else {
            Err(operation_error(
                if destroy {
                    "runtime.world.entity.destroy"
                } else {
                    "runtime.world.entity.place"
                },
                outcome,
            ))
        }
    }
    pub fn world_entity_destroy_handle(handle: u32) -> Result<(), String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        let mut outcome = 1u32;
        let ok = unsafe { (provider.world_entity_destroy_handle)(handle, &mut outcome) };
        if ok {
            Ok(())
        } else {
            Err(operation_error("runtime.world.entity.destroy", outcome))
        }
    }
    pub fn world_entity_finish_building(complete: bool) -> Result<(), String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        let mut outcome = 1u32;
        let ok = unsafe { (provider.world_entity_finish)(complete, &mut outcome) };
        if ok {
            Ok(())
        } else {
            Err(operation_error(
                "runtime.world.entity.finish_building",
                outcome,
            ))
        }
    }
    pub fn runtime_patch_available(name: &str) -> bool {
        let Some(provider) = provider() else {
            return false;
        };
        let Ok(name) = CString::new(name) else {
            return false;
        };
        unsafe { (provider.runtime_patch_available)(name.as_ptr()) }
    }
    pub fn runtime_patch_set_enabled(name: &str, enabled: bool) -> Result<(), String> {
        let Some(provider) = provider() else {
            return Err("KFC Runtime provider unavailable".into());
        };
        let name = CString::new(name).map_err(|_| "invalid patch name")?;
        let mut outcome = 1u32;
        let ok =
            unsafe { (provider.runtime_patch_set_enabled)(name.as_ptr(), enabled, &mut outcome) };
        if ok {
            return Ok(());
        }
        Err(format!(
            "runtime patch '{}' rejected the operation or failed its live-byte safety check (outcome={outcome})",
            name.to_string_lossy()
        ))
    }
    pub fn report() -> serde_json::Value {
        let Some(provider) = provider() else {
            return serde_json::json!({"available":false,"abi":observed_abi(),"reason":PROVIDER_ERROR.get().map(String::as_str).unwrap_or("provider-unavailable")});
        };
        let mut buffer = [0i8; 4096];
        unsafe {
            (provider.status)(buffer.as_mut_ptr(), buffer.len());
        }
        let bytes: Vec<u8> = buffer
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| *byte as u8)
            .collect();
        let detail = String::from_utf8_lossy(&bytes).into_owned();
        // The dispatcher exposes this explicit state before its first local
        // actor-world tick. Once that tick has occurred, its status changes to
        // ready or stale-game-thread, so a later pause is not mistaken for the
        // initial menu wait.
        let awaiting_world = detail.contains("game_thread=installed-awaiting-world");
        serde_json::json!({"available":true,"abi":provider.abi,"initialized":true,"ready":unsafe{(provider.ready)()},"writable":unsafe{(provider.can_write)()},"awaitingWorld":awaiting_world,"detail":detail})
    }
    pub fn resolve(name: &str) -> Option<Component> {
        let provider = provider()?;
        let name = CString::new(name).ok()?;
        let mut size = 0;
        unsafe { (provider.describe)(name.as_ptr(), &mut size) }.then_some(Component { size })
    }
    pub fn resolve_type(name: &str) -> Option<String> {
        let resolve = provider()?.resolve_type?;
        let name = CString::new(name).ok()?;
        let mut buffer = [0u8; 512];
        if !unsafe { resolve(name.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len()) } {
            return None;
        }
        let length = buffer.iter().position(|byte| *byte == 0)?;
        String::from_utf8(buffer[..length].to_vec()).ok()
    }
    pub fn registry() -> Result<serde_json::Value, String> {
        let fetch = provider().and_then(|p| p.registry).ok_or(
            "native provider lacks the component registry extension; install the matching provider",
        )?;
        fetch_json(fetch, 8 * 1024 * 1024)
    }
    pub fn functions() -> Result<&'static serde_json::Value, String> {
        static CATALOG: OnceLock<serde_json::Value> = OnceLock::new();
        if let Some(catalog) = CATALOG.get() {
            return Ok(catalog);
        }
        let fetch = provider()
            .and_then(|p| p.functions)
            .ok_or("native provider lacks the function inventory extension")?;
        let value = fetch_json(fetch, 32 * 1024 * 1024)?;
        let _ = CATALOG.set(value);
        Ok(CATALOG.get().unwrap())
    }
    pub fn function_code(rva: u32) -> Option<Vec<u8>> {
        let fetch = provider()?.function_code?;
        let mut code = vec![0; 512];
        let count = unsafe { fetch(rva, code.as_mut_ptr(), code.len()) };
        if count == 0 || count > code.len() {
            return None;
        }
        code.truncate(count);
        Some(code)
    }
    fn fetch_json(fetch: Registry, maximum: usize) -> Result<serde_json::Value, String> {
        let mut size = unsafe { fetch(std::ptr::null_mut(), 0) };
        for _ in 0..3 {
            if !(2..=maximum).contains(&size) {
                return Err("invalid native catalog response size".into());
            }
            let mut buffer = vec![0u8; size];
            let required = unsafe { fetch(buffer.as_mut_ptr().cast(), buffer.len()) };
            if required > size {
                size = required;
                continue;
            }
            if required < 2 || buffer[required - 1] != 0 {
                return Err("invalid component registry response".into());
            }
            return serde_json::from_slice(&buffer[..required - 1]).map_err(|e| e.to_string());
        }
        Err("component registry changed repeatedly during read".into())
    }
    pub fn query(components: &[String]) -> Result<Vec<u32>, &'static str> {
        let Some(provider) = provider() else {
            return Err("native ECS provider unavailable");
        };
        let Ok(names) = components
            .iter()
            .map(|name| CString::new(name.as_str()))
            .collect::<Result<Vec<_>, _>>()
        else {
            return Err("invalid ECS component name");
        };
        let pointers: Vec<_> = names.iter().map(|name| name.as_ptr()).collect();
        // Most player queries fit in one call. Do not count then rescan the
        // whole world on another engine tick for every query.
        let mut entities = vec![0; 256];
        for _ in 0..3 {
            let actual = unsafe {
                (provider.query)(
                    pointers.as_ptr(),
                    pointers.len(),
                    entities.as_mut_ptr(),
                    entities.len(),
                )
            };
            if actual == usize::MAX {
                return Err("native ECS query failed or timed out");
            }
            if actual == usize::MAX - 1 {
                return Err("live ECS query is still scanning; retry on the next update");
            }
            if actual <= entities.len() {
                entities.truncate(actual);
                return Ok(entities);
            }
            if actual > 1 << 20 {
                return Err("live ECS query returned an invalid entity count");
            }
            entities.resize(actual, 0);
        }
        Err("live ECS query buffer did not stabilize")
    }
    pub fn query_bounds(
        components: &[String],
        bounds: [f64; 6],
        padding: f64,
    ) -> Result<Vec<u32>, &'static str> {
        let Some(provider) = provider() else {
            return Err("native ECS provider unavailable");
        };
        let Some(query) = provider.query_bounds else {
            return Err("native spatial ECS query is unavailable; update KFC Runtime");
        };
        let Ok(names) = components
            .iter()
            .map(|name| CString::new(name.as_str()))
            .collect::<Result<Vec<_>, _>>()
        else {
            return Err("invalid ECS component name");
        };
        let pointers: Vec<_> = names.iter().map(|name| name.as_ptr()).collect();
        let mut entities = vec![0; 1 << 20];
        let actual = unsafe {
            query(
                pointers.as_ptr(),
                pointers.len(),
                bounds.as_ptr(),
                padding,
                entities.as_mut_ptr(),
                entities.len(),
            )
        };
        if actual == usize::MAX {
            return Err("native spatial ECS query failed or timed out");
        }
        if actual > entities.len() {
            return Err("native spatial ECS query exceeded the entity safety limit");
        }
        entities.truncate(actual);
        Ok(entities)
    }
    pub fn read(entity: u32, name: &str, size: u32) -> Option<Vec<u8>> {
        let provider = provider()?;
        let name = CString::new(name).ok()?;
        let mut bytes = vec![0; size as usize];
        unsafe {
            (provider.read)(
                entity,
                name.as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        }
        .then_some(bytes)
    }
    pub fn resolve_entity(entity_id: u32) -> Option<u32> {
        let provider = provider()?;
        let handle = unsafe { (provider.resolve_entity)(entity_id) };
        (handle != 0).then_some(handle)
    }
    pub fn write(entity: u32, name: &str, mask: &[u8], bytes: &[u8]) -> bool {
        let Some(provider) = provider() else {
            return false;
        };
        let Ok(name) = CString::new(name) else {
            return false;
        };
        if mask.len() != bytes.len() {
            return false;
        }
        unsafe {
            (provider.write)(
                entity,
                name.as_ptr(),
                mask.as_ptr().cast(),
                bytes.as_ptr().cast(),
                bytes.len(),
            )
        }
    }

    pub fn compare_exchange(
        entity: u32,
        name: &str,
        expected: &[u8],
        mask: &[u8],
        bytes: &[u8],
    ) -> Result<(), String> {
        let function = provider()
            .and_then(|p| p.compare_exchange)
            .ok_or("native runtime lacks guarded attribute updates")?;
        let name = CString::new(name).map_err(|e| e.to_string())?;
        if expected.len() != bytes.len() || mask.len() != bytes.len() {
            return Err("attribute update buffer sizes disagree".into());
        }
        match unsafe {
            function(
                entity,
                name.as_ptr(),
                expected.as_ptr().cast(),
                mask.as_ptr().cast(),
                bytes.as_ptr().cast(),
                bytes.len(),
            )
        } {
            1 => Ok(()),
            2 => Err(
                "attribute snapshot changed before update; read current values and retry".into(),
            ),
            _ => Err("guarded attribute update failed".into()),
        }
    }
}

#[cfg(not(windows))]
pub(super) mod runtime_provider {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct PropRecord {
        pub entity_handle: u32,
        pub item_id: u32,
        pub position: [i64; 3],
        pub orientation: [f32; 4],
        pub scale: [f32; 3],
        pub template_uuid: [u64; 2],
        pub entity_id: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct PropRecipe {
        pub item_id: u32,
        pub bounds: [f32; 6],
        pub feedback: u32,
    }
    #[derive(Clone, Copy)]
    pub struct Component {
        pub size: u32,
    }
    pub fn configure(_: &[(String, u32)]) -> bool {
        false
    }
    pub fn ready() -> bool {
        false
    }
    pub fn can_write() -> bool {
        false
    }
    pub fn world_operation_available(_: &str) -> bool {
        false
    }
    pub fn world_session_id() -> u64 {
        0
    }
    pub fn world_context_active() -> bool {
        false
    }
    pub fn world_entity_context_ready() -> bool {
        false
    }
    pub fn world_cursor_read() -> Option<(Vec<u8>, u64)> {
        None
    }
    pub fn world_voxel_read(_: [i32; 3], _: [u32; 3], _: usize) -> Option<Vec<u16>> {
        None
    }
    pub fn world_voxel_write(_: [i32; 3], _: [u32; 3], _: &[u16]) -> Result<(), String> {
        Err("native world runtime is available on Windows only".into())
    }
    pub fn world_entity_spawn(
        _: [u64; 2],
        _: [f64; 3],
        _: [f64; 4],
        _: u32,
        _: u32,
    ) -> Result<u32, String> {
        Err("native world runtime is available on Windows only".into())
    }
    pub fn world_entity_placement(
        _: bool,
        _: [f64; 3],
        _: [f64; 4],
        _: [f32; 6],
        _: u32,
        _: u32,
    ) -> Result<(), String> {
        Err("native world runtime is available on Windows only".into())
    }
    pub fn world_entity_query_props(_: [f64; 6], _: f64) -> Result<Vec<PropRecord>, &'static str> {
        Err("native world runtime is available on Windows only")
    }
    pub fn world_entity_register_prop_recipes(_: &[PropRecipe]) -> Result<(), String> {
        Err("native world runtime is available on Windows only".into())
    }
    pub fn world_entity_query_props_in_bounds(
        _: [f64; 6],
    ) -> Result<Vec<PropRecord>, &'static str> {
        Err("native world runtime is available on Windows only")
    }
    pub fn world_entity_get_transform(_: u32) -> Option<PropRecord> {
        None
    }
    pub fn world_entity_identity(_: u32) -> Option<u32> {
        None
    }
    pub fn world_entity_set_scale(_: u32, _: [f64; 3]) -> Result<(), String> {
        Err("native world runtime is available on Windows only".into())
    }
    pub struct GridSpecResult {
        pub id: String,
        pub origin: [f64; 3],
        pub cell_size: [f64; 3],
        pub maximum: [u64; 3],
    }
    pub fn world_grid_get_spec(_: &str) -> Option<GridSpecResult> {
        None
    }
    pub fn world_entity_destroy_handle(_: u32) -> Result<(), String> {
        Err("native world runtime is available on Windows only".into())
    }
    pub fn world_entity_finish_building(_: bool) -> Result<(), String> {
        Err("native world runtime is available on Windows only".into())
    }
    pub fn runtime_patch_available(_: &str) -> bool {
        false
    }
    pub fn runtime_patch_set_enabled(_: &str, _: bool) -> Result<(), String> {
        Err("native runtime patches are available on Windows only".into())
    }
    pub fn report() -> serde_json::Value {
        serde_json::json!({"available":false,"reason":"windows-runtime-only"})
    }
    pub fn registry() -> Result<serde_json::Value, String> {
        Err("native component registry is available on Windows only".into())
    }
    pub fn functions() -> Result<&'static serde_json::Value, String> {
        Err("native function inventory is available on Windows only".into())
    }
    pub fn function_code(_: u32) -> Option<Vec<u8>> {
        None
    }
    pub fn resolve_type(_: &str) -> Option<String> {
        None
    }
    pub fn resolve(_: &str) -> Option<Component> {
        None
    }
    pub fn query(_: &[String]) -> Result<Vec<u32>, &'static str> {
        Err("native ECS runtime is available on Windows only")
    }
    pub fn query_bounds(_: &[String], _: [f64; 6], _: f64) -> Result<Vec<u32>, &'static str> {
        Err("native ECS runtime is available on Windows only")
    }
    pub fn read(_: u32, _: &str, _: u32) -> Option<Vec<u8>> {
        None
    }
    pub fn resolve_entity(_: u32) -> Option<u32> {
        None
    }
    pub fn write(_: u32, _: &str, _: &[u8], _: &[u8]) -> bool {
        false
    }
    pub fn compare_exchange(_: u32, _: &str, _: &[u8], _: &[u8], _: &[u8]) -> Result<(), String> {
        Err("native ECS runtime is available on Windows only".into())
    }
}
