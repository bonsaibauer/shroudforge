use super::*;
pub(super) fn lua_get_content(
    lua: &mlua::Lua,
    args: FunctionArgs,
) -> mlua::Result<Option<Content>> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let guid = args.get::<ContentHash>(0)?;

    if !app_state.contains_content(&guid) {
        return Ok(None);
    }

    Ok(Some(Content::new(guid)))
}

pub(super) fn lua_get_all_contents(lua: &mlua::Lua, _args: FunctionArgs) -> mlua::Result<Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let file = app_state.kfc_file();
    let result = lua.create_table_with_capacity(file.contents().len(), 0)?;

    for guid in file.contents().keys() {
        result.push(Content::new(*guid))?;
    }

    Ok(result)
}

pub(super) fn lua_create_content(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<Content> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let buffer = args.get::<&Buffer>(0)?;

    let data = buffer.data()?;

    if data.len() > u32::MAX as usize {
        return Err(LuaError::generic("content size exceeds maximum limit"));
    }

    app_state.create_content(data).map(Content::new)
}
