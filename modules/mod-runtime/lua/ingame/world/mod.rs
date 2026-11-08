use super::*;
mod access_prop_transform;
mod change_building_entities;
mod query_props;
mod read_world_context;
mod read_write_voxels;
mod register_prop_recipes;
use access_prop_transform::*;
use change_building_entities::*;
use query_props::*;
use read_world_context::*;
use read_write_voxels::*;
use register_prop_recipes::*;
pub(super) fn create_world(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    // Share the reflected layout as the runtime API is created, so native
    // callers do not need a prior Lua cursor snapshot to initialize it.
    let state = lua.app_data_ref::<AppState>().unwrap();
    super::player_input::read_building_cursor::publish_local_camera_layout(&state);
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
    add_function_with_mod(lua, &world, "context_kind", r#mod, lua_world_context_kind)?;
    add_function_with_mod(lua, &world, "session_id", r#mod, lua_world_session_id)?;
    let voxel = lua.create_table()?;
    add_function_with_mod(lua, &voxel, "read", r#mod, lua_world_voxel_read)?;
    add_function_with_mod(lua, &voxel, "write", r#mod, lua_world_voxel_write)?;
    add_function_with_mod(lua, &voxel, "get_grid_spec", r#mod, lua_world_grid_get_spec)?;
    world.raw_set("voxel", voxel)?;
    let cursor = lua.create_table()?;
    add_function_with_mod(
        lua,
        &cursor,
        "get",
        r#mod,
        super::player_input::read_building_cursor::lua_world_cursor_get,
    )?;
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

fn prop_to_lua(lua: &mlua::Lua, prop: &runtime_provider::PropRecord) -> mlua::Result<mlua::Table> {
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
    Ok(item)
}
