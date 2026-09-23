use mlua::Table;
use mod_loader::Mod;

mod buffer;
mod game;
mod hasher;
mod image;
mod integer;
mod io;
mod loader;
mod log;
pub(crate) mod shroudforge;
mod table;

mod app_state;
mod util;

pub(crate) fn runtime_provider_report() -> serde_json::Value {
    loader::runtime_provider_report()
}

pub use app_state::*;

use buffer::Buffer;
#[allow(unused_imports)]
use game::{Content, Resource, Type, value};

pub fn register(lua: &mlua::Lua, env: &Table, r#mod: &Mod) -> mlua::Result<()> {
    let lua_builtin = lua.create_table()?;

    let lua_io = io::create(lua, r#mod.clone())?;
    let lua_integer = integer::create(lua)?;
    let lua_game = game::create(lua)?;
    let lua_buffer = buffer::create(lua)?;
    let lua_hasher = hasher::create(lua)?;
    let lua_runtime = loader::create(lua, r#mod.clone())?;
    let lua_loader = loader::create_eml(lua, r#mod)?;
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
