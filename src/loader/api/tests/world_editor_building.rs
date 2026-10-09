use mlua::Lua;

#[test]
fn game_building_queue_and_recipe_planner() -> mlua::Result<()> {
    let lua = Lua::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let module_dir = root.join("mods/world-editor/src");
    for name in ["building_plan", "game_building"] {
        let source = std::fs::read_to_string(module_dir.join(format!("{name}.lua"))).unwrap();
        let module: mlua::Value = lua.load(source).set_name(name).eval()?;
        lua.globals().set(name, module)?;
    }
    // Compile the whole editor too, to catch integration syntax errors.
    lua.load(std::fs::read_to_string(module_dir.join("mod.lua")).unwrap())
        .set_name("world-editor")
        .into_function()?;
    lua.load(include_str!("world_editor_building.lua")).exec()
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
#[ignore = "requires a fresh inspect_building_inputs export in SF_BUILDING_RESOURCE_SNAPSHOT"]
fn fresh_building_assets_resolve_without_hardcoded_item_ids() -> mlua::Result<()> {
    fn convert(lua: &Lua, value: &serde_json::Value) -> mlua::Result<mlua::Value> {
        use mlua::Value;
        Ok(match value {
            serde_json::Value::Null => Value::Nil,
            serde_json::Value::Bool(v) => Value::Boolean(*v),
            serde_json::Value::Number(v) => Value::Number(v.as_f64().unwrap()),
            serde_json::Value::String(v) => Value::String(lua.create_string(v)?),
            serde_json::Value::Array(values) => {
                let table = lua.create_table()?;
                for (i, v) in values.iter().enumerate() {
                    table.set(i + 1, convert(lua, v)?)?;
                }
                Value::Table(table)
            }
            serde_json::Value::Object(values) => {
                let table = lua.create_table()?;
                for (k, v) in values {
                    table.set(k.as_str(), convert(lua, v)?)?;
                }
                Value::Table(table)
            }
        })
    }
    let lua = Lua::new();
    let path =
        std::env::var("SF_BUILDING_RESOURCE_SNAPSHOT").expect("set fresh resource export path");
    let snapshot: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    lua.globals().set("snapshot", convert(&lua, &snapshot)?)?;
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../mods/world-editor/src/building_plan.lua");
    lua.globals().set(
        "planner",
        lua.load(std::fs::read_to_string(source).unwrap())
            .eval::<mlua::Table>()?,
    )?;
    lua.load(r#"
        local items,registries={},{}
        for _,r in ipairs(snapshot.resources) do
            if r.qualifiedType=='keen::ItemInfo' then items[#items+1]={data=r.value} end
            if r.qualifiedType=='keen::VoxelBlueprintItemRegistryResource' then registries[#registries+1]={data=r.value} end
        end
        assert(#items>0 and #registries>0)
        local recipes=assert(planner.catalog(items,registries,{.5,.5,.5}))
        local count=0
        for material,item in pairs(recipes) do assert(material>=128 and material<=255 and item>0); count=count+1 end
        assert(count>0,'fresh assets yielded no exact one-cell recipes')
        print('Fresh KFC: '..count..' exact one-cell building recipes, no fixed item IDs')
    "#).exec()
}
