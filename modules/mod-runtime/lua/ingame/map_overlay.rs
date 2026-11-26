use super::*;
use crate::lua::convert_json::{json_to_lua, lua_to_json};

pub(super) fn map_operation(feature: &str) -> bool {
    matches!(
        feature,
        "runtime.world.map.get" | "runtime.world.environment.get" | "runtime.world.hud.get"
    )
}
pub(super) fn overlay_operation(feature: &str) -> bool {
    matches!(
        feature,
        "runtime.overlay.submit" | "runtime.overlay.clear" | "runtime.overlay.status"
    )
}
pub(super) fn attach_world(
    lua: &mlua::Lua,
    world: &mlua::Table,
    package: &Mod,
) -> mlua::Result<()> {
    for (namespace, feature, view) in [
        ("map", "runtime.world.map.get", None),
        (
            "environment",
            "runtime.world.environment.get",
            Some("environment"),
        ),
        ("hud", "runtime.world.hud.get", Some("hud")),
    ] {
        let table = lua.create_table()?;
        add_function_with_mod(lua, &table, "get", package, move |lua, args, package| {
            snapshot(lua, args, package, feature, view)
        })?;
        world.raw_set(namespace, table)?;
    }
    Ok(())
}
pub(super) fn create_overlay(lua: &mlua::Lua, package: &Mod) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    add_function_with_mod(lua, &table, "submit", package, submit)?;
    add_function_with_mod(lua, &table, "clear", package, clear)?;
    add_function_with_mod(lua, &table, "status", package, status)?;
    Ok(table)
}
fn scope(lua: &mlua::Lua, package: &Mod, feature: &str) -> Result<u64, String> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, package, feature) {
        return Err(reason);
    }
    if feature != "runtime.overlay.status" {
        state.mark_runtime_services_used(&package.info().id);
    }
    state
        .runtime_package_generation(&package.info().id)
        .ok_or("runtime package scope inactive".into())
}
fn snapshot(
    lua: &mlua::Lua,
    args: FunctionArgs,
    package: &Mod,
    feature: &str,
    view: Option<&str>,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let generation = match scope(lua, package, feature) {
        Ok(value) => value,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    let expected = match args.get::<Option<String>>(0)? {
        None => 0,
        Some(value) => value.parse::<u64>().map_err(|_| {
            LuaError::generic("expected map session must be a decimal uint64 string")
        })?,
    };
    let frame = match runtime_provider::map_snapshot(
        &package.info().id,
        generation,
        expected,
        match view {
            Some("environment") => 1,
            Some("hud") => 2,
            _ => 0,
        },
    ) {
        Ok(value) => value,
        Err(reason) => return Ok((LuaValue::Nil, Some(reason))),
    };
    let value = match view {
        Some("environment") => frame["environment"].clone(),
        Some("hud") => {
            let supported = frame["supported_features"]
                .as_u64()
                .is_some_and(|bits| bits & 64 != 0);
            let status = match frame["status"].as_u64() {
                Some(0) if !supported => 2,
                Some(0) if frame["state_flags"].as_u64().unwrap_or(0) & 6 == 0 => 4,
                Some(value) => value,
                None => 9,
            };
            serde_json::json!({"status":status,"state_flags":frame["state_flags"],
            "session_generation":frame["session_generation"],"now_ms":frame["now_ms"],"clock_domain":frame["clock_domain"],
            "supported":supported})
        }
        _ => frame,
    };
    Ok((json_to_lua(lua, &value)?, None))
}
fn submit(
    lua: &mlua::Lua,
    args: FunctionArgs,
    package: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let generation = match scope(lua, package, "runtime.overlay.submit") {
        Ok(value) => value,
        Err(reason) => return Ok((false, Some(reason))),
    };
    let bytes = match args.get::<LuaValue>(0)? {
        LuaValue::String(text) => text.as_bytes().to_vec(),
        value @ LuaValue::Table(_) => {
            serde_json::to_vec(&lua_to_json(value.clone())?).map_err(LuaError::external)?
        }
        _ => {
            return Err(LuaError::generic(
                "overlay scene must be a table or UTF-8 JSON string",
            ));
        }
    };
    outcome(runtime_provider::submit_overlay_scene(
        &package.info().id,
        generation,
        &bytes,
    ))
}
fn clear(lua: &mlua::Lua, _: FunctionArgs, package: &Mod) -> mlua::Result<(bool, Option<String>)> {
    let generation = match scope(lua, package, "runtime.overlay.clear") {
        Ok(value) => value,
        Err(reason) => return Ok((false, Some(reason))),
    };
    outcome(runtime_provider::submit_overlay_scene(
        &package.info().id,
        generation,
        b"{}",
    ))
}
fn status(
    lua: &mlua::Lua,
    _: FunctionArgs,
    package: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    if let Err(reason) = scope(lua, package, "runtime.overlay.status") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    match runtime_provider::map_overlay_status() {
        Ok(value) => Ok((json_to_lua(lua, &value["overlay"])?, None)),
        Err(reason) => Ok((LuaValue::Nil, Some(reason))),
    }
}
fn outcome(result: Result<(), String>) -> mlua::Result<(bool, Option<String>)> {
    Ok(match result {
        Ok(()) => (true, None),
        Err(reason) => (false, Some(reason)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_factories_register_real_functions_and_deny_pregame_access() {
        let fixture = crate::session::asset_tests::Fixture::with_capabilities(&[
            "runtime",
            "runtime-map",
            "runtime-overlay",
        ]);
        let package = fixture
            .lua
            .app_data_ref::<AppState>()
            .unwrap()
            .env()
            .mod_registry()
            .get("fixture")
            .unwrap()
            .clone();
        let world = fixture.lua.create_table().unwrap();
        attach_world(&fixture.lua, &world, &package).unwrap();
        fixture.lua.globals().set("world", world).unwrap();
        fixture
            .lua
            .globals()
            .set("overlay", create_overlay(&fixture.lua, &package).unwrap())
            .unwrap();
        fixture.lua.load(r#"
            for _, name in ipairs({"map","environment","hud"}) do
                assert(type(world[name].get)=="function")
                local value,reason=world[name].get()
                assert(value==nil and reason:find("ingame phase",1,true))
            end
            for _, name in ipairs({"submit","clear","status"}) do assert(type(overlay[name])=="function") end
            local ok,reason=overlay.submit({})
            assert(ok==false and reason:find("ingame phase",1,true))
        "#).exec().unwrap();
    }
}
