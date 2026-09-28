use mlua::Table;
use mod_loader::Mod;

use crate::env::{AppState, loader::available};

/// Keeps the EML v1 `loader` namespace available to every compatible mod.
pub(crate) fn create_loader(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<Table> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let table = lua.create_table()?;
    table.raw_set("is_client", state.is_client())?;
    table.raw_set("is_server", state.is_server())?;
    table.raw_set(
        "has_mod",
        lua.create_function(|lua, id: String| {
            let state = lua.app_data_ref::<AppState>().unwrap();
            Ok(state.env().mod_registry().contains_key(&id))
        })?,
    )?;
    let features = lua.create_table()?;
    features.raw_set("patch", available(&state, r#mod, "game.assets.write"))?;
    features.raw_set("export", available(&state, r#mod, "export"))?;
    let runtime_features = lua.create_table()?;
    runtime_features.raw_set(
        "dll",
        state.has_feature(crate::env::AppFeatures::RUNTIME_DLL),
    )?;
    features.raw_set("runtime", runtime_features)?;
    table.raw_set("features", features)?;

    let runtime = lua.create_table()?;
    let target_mod = r#mod.clone();
    runtime.raw_set(
        "register_dll",
        lua.create_function(move |lua, path: String| {
            let state = lua.app_data_ref::<AppState>().unwrap();
            if !state.has_feature(crate::env::AppFeatures::RUNTIME_DLL) {
                return Err(mlua::Error::external(
                    "EML runtime DLL support is unavailable in this phase",
                ));
            }
            state
                .load_mod_native_dll(&target_mod, &path)
                .map_err(mlua::Error::external)
        })?,
    )?;
    table.raw_set("runtime", runtime)?;
    Ok(table)
}
