use mlua::{Lua, Value};

fn convert(lua: &Lua, value: &serde_json::Value) -> mlua::Result<Value> {
    Ok(match value {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(v) => Value::Boolean(*v),
        serde_json::Value::Number(v) => v
            .as_i64()
            .map(Value::Integer)
            .unwrap_or_else(|| Value::Number(v.as_f64().unwrap())),
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

fn check(snapshot: serde_json::Value) -> mlua::Result<()> {
    let lua = Lua::new();
    lua.globals().set("snapshot", convert(&lua, &snapshot)?)?;
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../mods/sf-production-time/src/mod.lua");
    lua.globals()
        .set("source", std::fs::read_to_string(source).unwrap())?;
    lua.load(r#"
        local function copy(v)
            if type(v)~='table' then return v end
            local r={} for k,x in pairs(v) do r[k]=copy(x) end return r
        end
        local function equal(a,b)
            if type(a)~='table' then return a==b end
            if type(b)~='table' then return false end
            for k,v in pairs(a) do if not equal(v,b[k]) then return false end end
            for k in pairs(b) do if a[k]==nil then return false end end
            return true
        end
        local stores, resources={},{}
        local writes=0
        for _,r in ipairs(snapshot.resources) do
            if r.qualifiedType=='keen::RecipeRegistryResource' then
                local store={data=copy(r.value)} stores[#stores+1]=store
                resources[#resources+1]=setmetatable({}, {
                    __index=function(_,k) assert(k=='data') return copy(store.data) end,
                    __newindex=function(_,k,v) assert(k=='data') store.data=copy(v); writes=writes+1 end,
                })
            end
        end
        local original=copy(stores)
        local seconds=1
        loader={features={patch=true}}
        shroudforge={settings={get=function(key) assert(key=='seconds') return seconds end}}
        game={types={get=function(name) assert(name=='keen::RecipeRegistryResource') return name end},
            assets={get_resources_by_type=function() return resources end}}
        local function run() assert(load(source))() end
        run()
        local changed=0
        for i,store in ipairs(stores) do
            local expected=copy(original[i].data)
            for _,recipe in ipairs(expected.recipes) do
                if recipe.craftingDuration.value>0 then recipe.craftingDuration.value=1000000000; changed=changed+1 end
            end
            assert(equal(store.data,expected),'changed fields beyond positive craftingDuration')
        end
        assert(changed>0)
        local first_writes=writes
        run(); assert(writes==first_writes,'repeated preparation must be idempotent')
        for _,bad in ipairs({0,-1,3601,'1',math.huge,0/0}) do
            seconds=bad; assert(not pcall(run),'invalid setting accepted'); assert(writes==first_writes)
        end
        seconds=1
        stores[#stores].data.recipes[#stores[#stores].data.recipes].craftingDuration.value=-1
        assert(not pcall(run),'invalid recipe accepted'); assert(writes==first_writes)
        print('Verified '..changed..' timed recipes; inputs, outputs, instant recipes and other fields preserved')
    "#).exec()
}

#[test]
fn production_time_preserves_recipe_semantics_and_is_idempotent() -> mlua::Result<()> {
    check(
        serde_json::json!({"resources":[{"qualifiedType":"keen::RecipeRegistryResource", "value":{
        "recipes":[
            {"recipeId":{"value":1},"craftingDuration":{"value":600000000000_i64},"input":[{"count":20}],"output":[{"count":10}]},
            {"recipeId":{"value":2},"craftingDuration":{"value":0},"input":[{"count":1}],"output":[{"count":1}]},
            {"recipeId":{"value":3},"craftingDuration":{"value":1500000000},"input":[{"count":10}],"output":[{"count":5}]}
        ]}}]}),
    )
}

#[test]
#[ignore = "requires fresh RecipeRegistryResource exports in SF_RECIPE_SNAPSHOTS, separated by semicolons"]
fn production_time_on_fresh_client_and_server_assets() -> mlua::Result<()> {
    for path in std::env::var("SF_RECIPE_SNAPSHOTS")
        .expect("set SF_RECIPE_SNAPSHOTS")
        .split(';')
    {
        check(serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())?;
    }
    Ok(())
}

#[test]
#[ignore = "copies SF_FACTORY_GAME_SOURCE into a private temporary directory and runs the real asset pipeline"]
fn production_time_real_kfc_roundtrip_on_isolated_copy() -> Result<(), Box<dyn std::error::Error>> {
    use kfc::{
        container::{KFCFile, KFCReader},
        reflection::{LookupKey, TypeRegistry},
        resource::value::Value,
    };
    use shroudforge_api::{GameFiles, GameParser, KfcParser, RunArgs, RunOptions, ShroudForgeApi};
    fn recipes(
        root: &std::path::Path,
        stem: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let registry = TypeRegistry::load_from_executable(root.join(format!("{stem}.exe")))?;
        let file = KFCFile::from_path(root.join(format!("{stem}.kfc")), false)?;
        let ty = registry
            .get_by_name(LookupKey::Qualified("keen::RecipeRegistryResource"))
            .unwrap();
        let ids = shroudforge_parser::kfc_format::resources_by_type(
            &file,
            &registry,
            "keen::RecipeRegistryResource",
        );
        assert_eq!(ids.len(), 1);
        let mut reader = KFCReader::new(root, stem)?.into_cursor()?;
        let mut bytes = Vec::new();
        assert!(reader.read_resource_into(&ids[0], &mut bytes)?);
        Ok(serde_json::to_value(Value::from_bytes(
            &registry, ty, &bytes,
        )?)?)
    }
    let source = std::path::PathBuf::from(std::env::var("SF_FACTORY_GAME_SOURCE")?);
    let server = source.join("enshrouded_server.exe").is_file();
    let stem = if server {
        "enshrouded_server"
    } else {
        "enshrouded"
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "sf-production-roundtrip-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root)?;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    for extension in ["exe", "kfc", "kfc_resources"] {
        let name = format!("{stem}.{extension}");
        std::fs::copy(source.join(&name), root.join(name))?;
    }
    let package =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../mods/sf-production-time");
    let destination = root.join("mods/sf-production-time");
    std::fs::create_dir_all(destination.join("src"))?;
    for name in ["mod.json", "src/mod.lua"] {
        std::fs::copy(package.join(name), destination.join(name))?;
    }
    let mut settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package.join("extended.mod.json"))?)?;
    settings["enabled"] = true.into();
    std::fs::write(
        destination.join("extended.mod.json"),
        serde_json::to_vec(&settings)?,
    )?;
    let mut expected = recipes(&root, stem)?;
    let mut count = 0;
    for recipe in expected["recipes"].as_array_mut().unwrap() {
        if recipe["craftingDuration"]["value"].as_i64().unwrap() > 0 {
            recipe["craftingDuration"]["value"] = 1_000_000_000_i64.into();
            count += 1;
        }
    }
    assert!(count > 0);
    let environment =
        mod_loader::ModEnvironment::load(root.to_str().unwrap()).map_err(|e| format!("{e:?}"))?;
    let (plan, errors) = environment.plan_report(server, shroudforge_api::API_VERSION);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(plan.len(), 1);
    assert!(plan[0].info().requires_pregame());
    let files = if server {
        GameFiles::server(&root)
    } else {
        GameFiles::client(&root)
    };
    let api = ShroudForgeApi::new(
        shroudforge_compatibility::Compatibility::default().resolve(KfcParser.parse(&files)?),
    );
    shroudforge_api::run(
        &environment,
        api,
        RunArgs {
            file_name: stem.into(),
            options: RunOptions {
                patch: true,
                force_patch: true,
                skip_cache: true,
                is_server: Some(server),
                ..Default::default()
            },
        },
    )?;
    assert_eq!(
        recipes(&root, stem)?,
        expected,
        "real KFC roundtrip changed unexpected recipe fields"
    );
    println!("Real Lua/KFC roundtrip passed for {stem}: {count} timed recipes; source installation unchanged");
    Ok(())
}
