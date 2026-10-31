use super::*;
pub(super) fn lua_get_resource(
    lua: &mlua::Lua,
    args: FunctionArgs,
) -> mlua::Result<Option<Resource>> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let guid = args.get::<Guid>(0)?;
    let r#type = get_type(&args, 1, app_state.type_registry().as_ref())?;
    let part = args.get::<Option<u32>>(2)?;

    let guid = kfc_format::resource_id(
        app_state.type_registry(),
        &r#type.qualified_name,
        guid,
        part.unwrap_or(0),
    )
    .ok_or_else(|| LuaError::generic(format!("type not found: {}", r#type.qualified_name)))?;

    Ok(app_state.get_resource_info(&guid).map(Resource::new))
}

pub(super) fn lua_get_resource_parts(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let guid = args.get::<Guid>(0)?;
    let r#type = get_type(&args, 1, app_state.type_registry().as_ref())?;

    let target_guid =
        kfc_format::resource_id(app_state.type_registry(), &r#type.qualified_name, guid, 0)
            .ok_or_else(|| {
                LuaError::generic(format!("type not found: {}", r#type.qualified_name))
            })?;

    let file = app_state.kfc_file();
    let result = lua.create_table()?;

    for guid in file.resources().keys() {
        if guid.guid() != target_guid.guid() || !kfc_format::same_type(guid, &target_guid) {
            continue;
        }

        let resource = match app_state.get_resource_info(guid) {
            Some(info) => Resource::new(info),
            None => {
                debug!("Resource info not found for GUID: {}", guid);
                continue;
            }
        };

        result.push(resource)?;
    }

    Ok(result)
}

pub(super) fn lua_get_resources_by_type(
    lua: &mlua::Lua,
    args: FunctionArgs,
) -> mlua::Result<Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let r#type = get_type(&args, 0, app_state.type_registry().as_ref())?;

    let file = app_state.kfc_file();
    let result = lua.create_table_with_capacity(file.resources().len(), 0)?;

    for guid in
        kfc_format::resources_by_type(file, app_state.type_registry(), &r#type.qualified_name)
    {
        let resource = match app_state.get_resource_info(&guid) {
            Some(info) => Resource::new(info),
            None => {
                debug!("Resource info not found for GUID: {}", guid);
                continue;
            }
        };

        result.push(resource)?;
    }

    // Mods can add ItemInfo resources before a later mod scans recipe outputs.
    // Include those session resources once, alongside the original KFC entries.
    for info in app_state.get_cached_resources() {
        if !file.resources().contains_key(&info.resource_id)
            && kfc_format::type_for_resource(app_state.type_registry(), &info.resource_id)
                .is_some_and(|metadata| metadata.index == r#type.index)
        {
            result.push(Resource::new(info))?;
        }
    }

    Ok(result)
}

pub(super) fn lua_get_all_resources(lua: &mlua::Lua, _args: FunctionArgs) -> mlua::Result<Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let file = app_state.kfc_file();
    let result = lua.create_table_with_capacity(file.resources().len(), 0)?;

    for guid in file.resources().keys() {
        let resource = match app_state.get_resource_info(guid) {
            Some(info) => Resource::new(info),
            None => {
                debug!("Resource info not found for GUID: {}", guid);
                continue;
            }
        };

        result.push(resource)?;
    }

    Ok(result)
}

pub(super) fn lua_get_resource_types(lua: &mlua::Lua, _args: FunctionArgs) -> mlua::Result<Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let file = app_state.kfc_file();
    let result = lua.create_table_with_capacity(file.resource_bundles().len(), 0)?;

    for type_index in kfc_format::resource_type_indices(file, app_state.type_registry()) {
        let value = app_state
            .get_type(lua, type_index)?
            .expect("type not found in context");

        result.push(value)?;
    }

    Ok(result)
}

pub(super) fn lua_create_resource(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<Resource> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    let value = args.get::<LuaValue>(0)?;
    let r#type = get_type(&args, 1, app_state.type_registry().as_ref())?;

    let guid = if args.len() > 2 {
        let guid = args.get::<Guid>(2)?;
        let part = args.get::<u32>(3)?;

        let guid = kfc_format::resource_id(
            app_state.type_registry(),
            &r#type.qualified_name,
            guid,
            part,
        )
        .ok_or_else(|| LuaError::generic(format!("type not found: {}", r#type.qualified_name)))?;

        app_state.add_resource(value, &guid, lua)?;

        guid
    } else {
        app_state.create_resource(value, r#type.index(), lua)?
    };

    Ok(app_state
        .get_resource_info(&guid)
        .map(Resource::new)
        .expect("just created resource info must exist"))
}
