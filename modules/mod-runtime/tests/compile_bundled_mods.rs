use mlua::Lua;

#[test]
fn bundled_lua_compiles() -> mlua::Result<()> {
    let lua = Lua::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods");
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
fn crafting_scale_updates_loaded_recipes_and_restores_originals() -> mlua::Result<()> {
    let lua = Lua::new();
    lua.globals().set(
        "crafting_source",
        include_str!("../../../mods/sf-crafting-essentials/src/mod.lua"),
    )?;
    lua.globals().set(
        "overrides_source",
        include_str!("../lua/ingame/overrides/track_runtime_overrides.lua"),
    )?;
    let targets: mlua::Function = lua
        .load(include_str!(
            "../lua/ingame/overrides/create_override_targets.lua"
        ))
        .eval()?;
    lua.globals().set("override_targets_factory", targets)?;
    lua.load(include_str!("crafting_lifecycle.lua"))
        .set_name("crafting-live-production-scale")
        .exec()
}

#[test]
fn world_editor_preserves_blueprints_and_blocks_unavailable_world_changes() -> mlua::Result<()> {
    let lua = Lua::new();
    let sources = lua.create_table()?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mods/world-editor/src");
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "lua") {
            sources.set(
                path.file_stem().unwrap().to_str().unwrap(),
                std::fs::read_to_string(&path).unwrap(),
            )?;
        }
    }
    lua.globals().set("world_editor_sources", sources)?;
    lua.globals().set(
        "world_editor_v8_fixture",
        include_str!("../../../mods/world-editor/schemas/8/manifest.example.json"),
    )?;
    let extension: serde_json::Value =
        serde_json::from_str(include_str!("../../../mods/world-editor/extended.mod.json")).unwrap();
    let settings = lua.create_table()?;
    for (name, definition) in extension["settings"].as_object().unwrap() {
        match &definition["value"] {
            serde_json::Value::String(value) => settings.set(name.as_str(), value.as_str())?,
            serde_json::Value::Number(value) => {
                settings.set(name.as_str(), value.as_f64().unwrap())?
            }
            value => panic!("unexpected World Editor setting {name}: {value}"),
        }
    }
    lua.globals().set("world_editor_settings", settings)?;
    lua.load(include_str!("world_editor_lifecycle.lua"))
        .set_name("world-editor-blueprints-and-world-gates")
        .exec()
}
