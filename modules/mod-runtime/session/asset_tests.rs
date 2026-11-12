use super::*;
use crate::env::{Buffer, game};
use kfc::container::StaticMapBuilder;
use kfc::guid::Guid;

// Synthetic containers and reflected types only. No game files or native host.
struct Fixture {
    lua: mlua::Lua,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("sf-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("mods/fixture/src")).unwrap();
        std::fs::write(
            root.join("mods/fixture/mod.json"),
            r#"{
            "id":"fixture", "name":"Fixture", "version":"1.0.0",
            "authors":["Test"], "description":"Synthetic assets", "capabilities":["patch"]
        }"#,
        )
        .unwrap();
        std::fs::write(root.join("mods/fixture/src/mod.lua"), "return {}").unwrap();
        let env = ModEnvironment::load(camino::Utf8Path::from_path(&root).unwrap()).unwrap();
        let owner = env
            .mod_registry()
            .get("fixture")
            .expect("fixture mod")
            .clone();
        let registry = Rc::new(game::resolve_type::tests::fixture());
        let mut file = KFCFile::default();
        let original =
            kfc_format::resource_id(&registry, "test::Base", Guid::from([1; 16]), 0).unwrap();
        let mut entries = StaticMapBuilder::default();
        entries.insert(original, Default::default());
        file.set_resources(entries.build(), &registry, None)
            .unwrap();
        let file = Rc::new(file);
        let api = ShroudForgeApi::new(
            shroudforge_compatibility::Compatibility::new(Vec::<String>::new())
                .resolve(kfc::schema_from_registry(&registry, &file)),
        );
        std::fs::write(root.join("fixture.kfc"), []).unwrap();
        std::fs::write(root.join("fixture.kfc_resources"), []).unwrap();
        let writer = KFCWriter::new(&root, "fixture", registry.clone(), "fixture").unwrap();
        let state = AppState {
            runtime_configured: Cell::new(false),
            runtime_active_mods: RefCell::default(),
            runtime_effects: RefCell::default(),
            runtime_write_fault: RefCell::new(None),
            runtime_ownership: Rc::new(RefCell::new(Default::default())),
            env,
            api,
            config: AppConfig {
                is_server: false,
                phase: RuntimePhase::Pregame,
                feature_flags: AppFeatures::PATCH,
                export_dir: PathBuf::from_path_buf(root.clone()).unwrap(),
            },
            type_registry: registry,
            attribute_catalog: OnceCell::new(),
            inline_fields: RefCell::default(),
            inline_field_bytes: Cell::new(0),
            ref_file: file,
            reader: RefCell::new(None),
            writer: RefCell::new(Some(writer)),
            asset_transaction: Cell::new(false),
            asset_stem: "fixture".into(),
            new_contents: RefCell::default(),
            content_reads_in_flight: Arc::new(AtomicUsize::new(0)),
            resources: RefCell::default(),
            loaded_resource_indexes: RefCell::new(Default::default()),
            types: RefCell::default(),
        };
        let lua = mlua::Lua::new();
        lua.set_app_data(state);
        lua.globals()
            .set("game", game::create(&lua, &owner).unwrap())
            .unwrap();
        Self { lua, root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.lua.remove_app_data::<AppState>();
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn asset_session_content_is_resolvable_before_publication() {
    let fixture = Fixture::new();
    let lua = &fixture.lua;
    lua.globals()
        .set("payload", Buffer::wrap(vec![11, 22, 33]))
        .unwrap();
    lua.globals()
        .set("missing", ContentHash::from_data(b"absent").to_string())
        .unwrap();
    lua.load(
        r#"
        local content = game.assets.create_content(payload)
        local resolved = assert(game.assets.get_content(content.guid))
        assert(resolved.guid == content.guid and resolved.size == 3)
        assert(game.assets.get_content(missing) == nil)
        local again = game.assets.create_content(payload)
        assert(again.guid == resolved.guid)
        resolved:read_data()
    "#,
    )
    .exec()
    .unwrap();
    let state = lua.app_data_ref::<AppState>().unwrap();
    let guid = ContentHash::from_data(&[11, 22, 33]);
    assert_eq!(state.get_content(&guid).unwrap().unwrap(), [11, 22, 33]);
    assert_eq!(state.new_contents.borrow().len(), 1);
    assert!(!state.kfc_file().contents().contains_key(&guid));
    // Retain 1.8.0's released-reader failure for data outside the session.
    assert!(
        state
            .get_content(&ContentHash::from_data(b"absent"))
            .is_err()
    );
}

#[test]
fn asset_session_type_scan_includes_new_resources_without_duplicates_or_loaded_snapshots() {
    let fixture = Fixture::new();
    let lua = &fixture.lua;
    {
        let state = lua.app_data_ref::<AppState>().unwrap();
        let id =
            kfc_format::resource_id(state.type_registry(), "test::Base", Guid::from([2; 16]), 0)
                .unwrap();
        state
            .resources
            .borrow_mut()
            .insert(id, Rc::new(ResourceInfo::loaded(id, 42)));
    }
    lua.load(
        r#"
        assert(#game.assets.get_resources_by_type('test::Base') == 1)
        local added = game.assets.create_resource({x=7}, 'test::Base')
        game.assets.create_resource({x=8,y=9}, 'test::Child')
        for i=1,2 do
            local found = game.assets.get_resources_by_type('test::Base')
            assert(#found == 2)
            local seen = {}
            for _, resource in ipairs(found) do
                assert(not seen[resource.guid] and not resource.loaded)
                seen[resource.guid] = true
            end
            assert(seen[added.guid])
        end
        assert(#game.assets.get_resources_by_type('test::Child') == 1)
    "#,
    )
    .exec()
    .unwrap();
}

#[test]
fn asset_session_owned_fields_use_the_field_type_and_preserve_values_on_rejection() {
    let fixture = Fixture::new();
    fixture
        .lua
        .load(
            r#"
        local resource = game.assets.create_resource({x=7,y=9}, 'test::Child')
        resource.data.x = 12
        resource.data.y = 34
        assert(resource.data.x == 12 and resource.data.y == 34)
        assert(not pcall(function() resource.data.x = 'wrong type' end))
        assert(not pcall(function() resource.data.missing = 1 end))
        assert(resource.data.x == 12 and resource.data.y == 34)
    "#,
        )
        .exec()
        .unwrap();
}
