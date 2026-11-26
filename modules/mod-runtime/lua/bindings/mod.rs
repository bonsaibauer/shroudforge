use mlua::Table;
use mod_loader::Mod;

pub(crate) mod buffer;
pub(crate) mod game;
#[path = "hash_bytes.rs"]
mod hasher;
mod image;
pub(crate) use image::encode_in_strips;
#[path = "integer_operations.rs"]
mod integer;
mod io;
#[path = "write_mod_log.rs"]
mod log;
#[path = "register_functions.rs"]
pub(crate) mod registration;
#[path = "extend_table_operations.rs"]
mod table;
pub(crate) use crate::lua::ingame as loader;
pub(crate) use crate::lua::ingame::access_named_service as runtime_services;
pub(crate) use crate::lua::ingame::ecs::{
    access_template_component as runtime_templates, attributes as runtime_attributes,
};
pub(crate) use crate::lua::ingame::functions as runtime_functions;
pub(crate) use crate::lua::ingame::overrides::{
    claim_runtime_storage as runtime_ownership, track_runtime_overrides as runtime_overrides,
};
pub(crate) use crate::lua::ingame::player_input::submit_building_input as runtime_building;
pub(crate) use crate::lua::resource_values as value;
pub(crate) use crate::lua::shroudforge::v1 as shroudforge;
pub(crate) use crate::lua::values as runtime_values;
pub(crate) use crate::session::*;
pub(crate) use game::resolve_type as registry;
pub(crate) use game::resolve_type as type_lookup;
pub(crate) mod runtime_networking {
    pub(crate) fn create(lua: &mlua::Lua, owner: &mod_loader::Mod) -> mlua::Result<mlua::Table> {
        shroudforge_steam_networking::lua::create(
            lua,
            owner,
            shroudforge_steam_networking::lua::RuntimeContext {
                root: |lua| {
                    lua.app_data_ref::<super::AppState>()
                        .unwrap()
                        .env()
                        .game_dir()
                        .as_std_path()
                        .to_owned()
                },
                is_server: |lua| lua.app_data_ref::<super::AppState>().unwrap().is_server(),
                denial_reason: |lua, owner, operation| {
                    let state = lua.app_data_ref::<super::AppState>().unwrap();
                    super::loader::runtime_denial_reason(&state, owner, operation)
                },
            },
        )
    }
}

pub(crate) fn runtime_provider_report() -> serde_json::Value {
    loader::runtime_provider_report()
}

pub(crate) use buffer::Buffer;
#[allow(unused_imports)]
pub(crate) use game::{Content, Resource, Type};

pub fn register(lua: &mlua::Lua, env: &Table, r#mod: &Mod) -> mlua::Result<()> {
    let lua_builtin = lua.create_table()?;

    let lua_io = io::create(lua, r#mod.clone())?;
    let lua_integer = integer::create(lua)?;
    let lua_game = game::create(lua, r#mod)?;
    let lua_buffer = buffer::create(lua)?;
    let lua_hasher = hasher::create(lua)?;
    let lua_runtime = loader::create(lua, r#mod.clone())?;
    lua_runtime.raw_set("types", lua_game.raw_get::<Table>("types")?)?;
    lua_runtime.raw_set(
        "values",
        runtime_values::create(
            lua,
            lua.app_data_ref::<AppState>()
                .unwrap()
                .type_registry()
                .clone(),
        )?,
    )?;
    crate::lua::ingame::overrides::attach(lua, &lua_runtime, &lua_game, r#mod)?;
    let lua_loader = crate::eml::v1::create_loader(lua, r#mod)?;
    let lua_shroudforge = shroudforge::create(lua, r#mod)?;
    let lua_image = image::create(lua)?;

    lua_builtin.raw_set("io", lua_io.clone())?;
    lua_builtin.raw_set("integer", lua_integer.clone())?;
    lua_builtin.raw_set("game", lua_game.clone())?;
    lua_builtin.raw_set("buffer", lua_buffer.clone())?;
    lua_builtin.raw_set("hasher", lua_hasher.clone())?;
    lua_builtin.raw_set("runtime", lua_runtime.clone())?;
    lua_builtin.raw_set("loader", lua_loader.clone())?;
    lua_builtin.raw_set("shroudforge", lua_shroudforge.clone())?;
    lua_builtin.raw_set("image", lua_image.clone())?;

    env.raw_set("builtin", lua_builtin)?;
    env.raw_set("io", lua_io)?;
    env.raw_set("integer", lua_integer)?;
    env.raw_set("game", lua_game)?;
    env.raw_set("buffer", lua_buffer)?;
    env.raw_set("hasher", lua_hasher)?;
    env.raw_set("runtime", lua_runtime)?;
    env.raw_set("loader", lua_loader)?;
    env.raw_set("shroudforge", lua_shroudforge)?;
    env.raw_set("image", lua_image)?;

    table::modify(&env.raw_get::<Table>("table")?, lua)?;
    log::register(lua, env, &r#mod.info().id)?;

    Ok(())
}
