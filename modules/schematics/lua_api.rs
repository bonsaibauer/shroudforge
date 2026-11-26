use mlua::{Function, Lua, String as LuaString, Table};
use std::path::PathBuf;

fn export_root(authorize: &Function) -> mlua::Result<PathBuf> {
    authorize.call::<String>(()).map(PathBuf::from)
}

pub fn create(lua: &Lua, authorize: Function) -> mlua::Result<Table> {
    let schematics = lua.create_table()?;
    schematics.raw_set(
        "list",
        lua.create_function({
            let authorize = authorize.clone();
            move |lua, ()| {
                let root = export_root(&authorize)?;
                let items = crate::schematic_library::list(&root).map_err(mlua::Error::runtime)?;
                let result = lua.create_table()?;
                for (index, schematic) in items.into_iter().enumerate() {
                    let item = lua.create_table()?;
                    item.raw_set("name", schematic.name)?;
                    item.raw_set("image", schematic.image)?;
                    item.raw_set("source", schematic.source)?;
                    item.raw_set("project_id", schematic.project_id)?;
                    item.raw_set("version_id", schematic.version_id)?;
                    item.raw_set("valid", schematic.valid)?;
                    item.raw_set("size", schematic.size)?;
                    item.raw_set("modified_at", schematic.modified_at)?;
                    result.raw_set(index + 1, item)?;
                }
                Ok(result)
            }
        })?,
    )?;
    schematics.raw_set(
        "read",
        lua.create_function({
            let authorize = authorize.clone();
            move |lua, name: String| {
                let root = export_root(&authorize)?;
                let bytes =
                    crate::schematic_library::read(&root, &name).map_err(mlua::Error::runtime)?;
                lua.create_string(bytes)
            }
        })?,
    )?;
    schematics.raw_set(
        "save",
        lua.create_function({
            let authorize = authorize.clone();
            move |_, (name, data): (String, LuaString)| {
                let root = export_root(&authorize)?;
                crate::schematic_library::save(&root, &name, data.as_bytes().as_ref())
                    .map_err(mlua::Error::runtime)
            }
        })?,
    )?;
    schematics.raw_set(
        "rename",
        lua.create_function({
            let authorize = authorize.clone();
            move |_, (name, new_name): (String, String)| {
                let root = export_root(&authorize)?;
                crate::schematic_library::rename(&root, &name, &new_name)
                    .map_err(mlua::Error::runtime)
            }
        })?,
    )?;
    schematics.raw_set(
        "duplicate",
        lua.create_function({
            let authorize = authorize.clone();
            move |_, (name, new_name): (String, String)| {
                let root = export_root(&authorize)?;
                crate::schematic_library::duplicate(&root, &name, &new_name)
                    .map_err(mlua::Error::runtime)
            }
        })?,
    )?;
    schematics.raw_set(
        "delete",
        lua.create_function({
            let authorize = authorize.clone();
            move |_, name: String| {
                let root = export_root(&authorize)?;
                crate::schematic_library::delete(&root, &name).map_err(mlua::Error::runtime)
            }
        })?,
    )?;
    schematics.raw_set(
        "set_cover",
        lua.create_function({
            let authorize = authorize.clone();
            move |_, (name, data): (String, LuaString)| {
                let root = export_root(&authorize)?;
                crate::schematic_library::set_cover(&root, &name, data.as_bytes().as_ref())
                    .map_err(mlua::Error::runtime)
            }
        })?,
    )?;
    schematics.raw_set(
        "undo_cover",
        lua.create_function(move |_, name: String| {
            let root = export_root(&authorize)?;
            crate::schematic_library::undo_cover(&root, &name).map_err(mlua::Error::runtime)
        })?,
    )?;
    Ok(schematics)
}
