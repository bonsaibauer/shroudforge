use mlua::Lua;

#[test]
fn world_editor_p2p_transport() -> mlua::Result<()> {
    let lua = Lua::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let module_dir = root.join("mods/world-editor/src");
    let factory: mlua::Value = lua
        .load(std::fs::read_to_string(module_dir.join("p2p.lua")).unwrap())
        .set_name("world-editor-p2p")
        .eval()?;
    lua.globals().set("p2p_factory", factory)?;
    lua.load(std::fs::read_to_string(module_dir.join("mod.lua")).unwrap())
        .set_name("world-editor")
        .into_function()?;
    lua.load(include_str!("world_editor_p2p.lua"))
        .set_name("world_editor_p2p.lua")
        .exec()
}

#[test]
fn server_target_applies_and_undoes_through_native_world_api() -> mlua::Result<()> {
    let lua = Lua::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let module_dir = root.join("mods/world-editor/src");
    let factory: mlua::Value = lua
        .load(std::fs::read_to_string(module_dir.join("p2p.lua")).unwrap())
        .set_name("world-editor-p2p")
        .eval()?;
    lua.globals().set("p2p_factory", factory)?;
    lua.globals().set(
        "editor_source",
        std::fs::read_to_string(module_dir.join("mod.lua")).unwrap(),
    )?;
    let builtin_require: mlua::Value = lua.globals().get("require")?;
    lua.globals().set("builtin_require", builtin_require)?;
    lua.load(include_str!("world_editor_server.lua"))
        .set_name("world_editor_server.lua")
        .exec()
}

#[test]
fn bundled_lua_compiles() -> mlua::Result<()> {
    let lua = Lua::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods");
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.path().extension().is_some_and(|ext| ext == "lua") {
            lua.load(std::fs::read_to_string(entry.path()).unwrap())
                .set_name(entry.path().to_string_lossy())
                .into_function()?;
        }
    }
    Ok(())
}

#[test]
fn blueprint_save_progress_confirms_write_and_library() -> mlua::Result<()> {
    // Debug upvalues are used only by this isolated regression fixture.
    let lua = unsafe { Lua::unsafe_new() };
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../mods/world-editor/src/mod.lua");
    lua.globals()
        .set("editor_source", std::fs::read_to_string(source).unwrap())?;
    lua.load(include_str!("world_editor_save_progress.lua"))
        .exec()
}
