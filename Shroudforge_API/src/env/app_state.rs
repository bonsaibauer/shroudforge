use std::{
    cell::{Cell, RefCell, RefMut},
    collections::{
        HashMap,
        hash_map::{self, Entry},
    },
    rc::Rc,
};

use bitflags::bitflags;
use kfc::{
    container::{KFCCursor, KFCFile, KFCReader, KFCWriter},
    guid::{ContentHash, ResourceId},
    reflection::{TypeHandle, TypeIndex, TypeRegistry},
    resource::value::Value,
};
use mod_loader::ModEnvironment;
use once_cell::unsync::OnceCell;
use shroudforge_parser::kfc_format;

use crate::{
    RunArgs, RuntimePhase, ShroudForgeApi,
    alias::{MappedValue, PathBuf},
    cache::CacheDiff,
    env::{
        Type,
        game::value::is_dirty_lua_value,
        value::{convert_lua_to_value, convert_value_to_lua, validate_and_clone_lua_value},
    },
    log::warn,
    lua::{LuaError, LuaValue},
};

pub struct AppState {
    pub(crate) runtime_configured: Cell<bool>,
    env: ModEnvironment,
    api: ShroudForgeApi,
    config: AppConfig,

    type_registry: Rc<TypeRegistry>,

    ref_file: Rc<KFCFile>,
    reader: RefCell<KFCCursor<KFCReader>>,
    writer: RefCell<Option<KFCWriter<Rc<KFCFile>, Rc<TypeRegistry>>>>,
    asset_transaction: Cell<bool>,
    asset_stem: String,

    // NOTE: the `Vec<u8>` must always be treated as immutable
    new_contents: RefCell<HashMap<ContentHash, Vec<u8>>>,
    resources: RefCell<HashMap<ResourceId, Rc<ResourceInfo>>>,
    types: RefCell<HashMap<TypeIndex, LuaValue>>,
}

pub struct AppConfig {
    skip_cache: bool,
    is_server: bool,
    phase: RuntimePhase,

    feature_flags: AppFeatures,
    export_dir: PathBuf,
}

bitflags! {
    pub struct AppFeatures: u32 {
        const ASSETS_WRITE = 1 << 0;
        const EXPORT = 1 << 1;

        const ALL = !0;
    }
}

impl AppState {
    pub fn new(
        env: ModEnvironment,
        api: Option<ShroudForgeApi>,
        args: RunArgs,
        cache_diff: &CacheDiff,
    ) -> Result<Self, ()> {
        let game_dir = env.game_dir();
        let cache_dir = env.cache_dir();
        let file_name = &args.file_name;

        // prepare configuration

        let options = args.options;
        let export_dir = options
            .export_dir
            .map(|d| d.canonicalize())
            .transpose()
            .map_err(|e| {
                warn!(
                    error = %e,
                    "Failed to canonicalize export directory, using default instead"
                );
            })?
            .map(|d| PathBuf::from_path_buf(d))
            .transpose()
            .map_err(|_| {
                warn!("Export directory is not valid UTF-8, using default instead");
            })?
            .unwrap_or_else(|| env.game_dir().join("export"));

        let skip_cache = options.skip_cache;
        let phase = options.phase;
        let is_server = options
            .is_server
            .unwrap_or_else(|| file_name.to_lowercase().contains("server"));

        let mut feature_flags = AppFeatures::empty();

        if options.assets_write {
            feature_flags |= AppFeatures::ASSETS_WRITE;
        }

        if options.export {
            feature_flags |= AppFeatures::EXPORT;
        }

        let config = AppConfig {
            skip_cache,
            export_dir,

            is_server,
            phase,

            feature_flags,
        };

        // load and attach to resources

        let (type_registry, is_dirty) =
            crate::load::load_type_registry(game_dir, cache_dir, file_name)?;
        if phase == RuntimePhase::Pregame {
            crate::load::export_lua_definitions(
                cache_dir,
                &type_registry,
                options.skip_cache || is_dirty || cache_diff.build_id_changed(),
            );
        }
        let type_registry = Rc::new(type_registry);

        let kfc_path = game_dir.join(file_name).with_extension("kfc");
        let (ref_file, reader) = if phase == RuntimePhase::Ingame {
            (
                crate::load::load_kfc_file(&kfc_path)?,
                crate::load::create_reader_with_extension(game_dir, file_name, "kfc")?,
            )
        } else {
            let bak_path = crate::load::create_backup(&kfc_path)?;
            (
                crate::load::load_kfc_file(&bak_path)?,
                crate::load::create_reader(game_dir, file_name)?,
            )
        };
        let api = match api {
            Some(api) => api,
            None => {
                let operations =
                    crate::runtime_operations(game_dir.as_std_path()).map_err(|error| {
                        warn!(%error, "Invalid installed API contract");
                    })?;
                let schema = shroudforge_parser::schema_from_registry(&type_registry, &ref_file);
                ShroudForgeApi::new(
                    shroudforge_compatibility::Compatibility::new(operations).resolve(schema),
                )
            }
        };
        let writer = if options.assets_write {
            let stage = shroudforge_parser::transaction::begin(game_dir.as_std_path(), file_name)
                .map_err(|error| {
                warn!(error = %error, "Failed to begin typed asset transaction");
            })?;
            let staged_writer = (|| {
                for suffix in ["kfc", "kfc_resources"] {
                    let name = format!("{file_name}.{suffix}");
                    std::fs::copy(game_dir.join(&name), stage.join(&name)).map_err(|error| {
                        warn!(error = %error, file = %name, "Failed to stage game asset container");
                    })?;
                }
                let stage = PathBuf::from_path_buf(stage.clone()).map_err(|_| {
                    warn!("Asset staging path is not valid UTF-8");
                })?;
                crate::load::create_writer(&stage, file_name, &type_registry, &ref_file)
            })();
            match staged_writer {
                Ok(writer) => Some(writer),
                Err(()) => {
                    if let Err(error) =
                        shroudforge_parser::transaction::abort(game_dir.as_std_path(), file_name)
                    {
                        warn!(error = %error, "Failed to roll back asset initialization");
                    }
                    return Err(());
                }
            }
        } else {
            None
        };

        Ok(Self {
            runtime_configured: Cell::new(false),
            env,
            api,
            config,

            type_registry,

            ref_file,
            reader: RefCell::new(reader),
            writer: RefCell::new(writer),
            asset_transaction: Cell::new(options.assets_write),
            asset_stem: file_name.clone(),

            new_contents: RefCell::new(HashMap::new()),
            resources: RefCell::new(HashMap::new()),
            types: RefCell::new(HashMap::new()),
        })
    }

    #[inline]
    pub fn env(&self) -> &ModEnvironment {
        &self.env
    }

    #[inline]
    pub fn api(&self) -> &ShroudForgeApi {
        &self.api
    }

    #[inline]
    #[allow(unused)]
    pub fn skip_cache(&self) -> bool {
        self.config.skip_cache
    }

    #[inline]
    pub fn is_server(&self) -> bool {
        self.config.is_server
    }

    #[inline]
    pub fn is_client(&self) -> bool {
        !self.config.is_server
    }

    #[inline]
    pub fn phase(&self) -> RuntimePhase {
        self.config.phase
    }

    #[inline]
    pub fn export_dir(&self) -> &PathBuf {
        &self.config.export_dir
    }

    #[inline]
    pub fn has_feature(&self, feature: AppFeatures) -> bool {
        self.config.feature_flags.contains(feature)
    }

    #[inline]
    pub fn kfc_file(&self) -> &KFCFile {
        &self.ref_file
    }

    #[inline]
    pub fn type_registry(&self) -> &Rc<TypeRegistry> {
        &self.type_registry
    }

    #[inline]
    pub fn reader(&self) -> RefMut<'_, KFCCursor<KFCReader>> {
        self.reader.borrow_mut()
    }

    #[inline]
    pub fn take_writer(&self) -> mlua::Result<KFCWriter<Rc<KFCFile>, Rc<TypeRegistry>>> {
        self.writer
            .borrow_mut()
            .take()
            .ok_or_else(|| LuaError::generic("typed asset writer is unavailable in this phase"))
    }

    pub fn commit_assets(&self) -> mlua::Result<()> {
        if !self.asset_transaction.get() {
            return Err(LuaError::generic("typed asset transaction is unavailable"));
        }
        shroudforge_parser::transaction::commit(
            self.env.game_dir().as_std_path(),
            &self.asset_stem,
        )
        .map_err(LuaError::external)?;
        self.asset_transaction.set(false);
        Ok(())
    }

    pub fn get_cached_resources(&self) -> Vec<Rc<ResourceInfo>> {
        self.resources.borrow().values().cloned().collect()
    }

    pub fn get_resource_info(&self, guid: &ResourceId) -> Option<Rc<ResourceInfo>> {
        match self.resources.borrow_mut().entry(*guid) {
            Entry::Occupied(entry) => Some(entry.get().clone()),
            Entry::Vacant(entry) => {
                if !self.ref_file.resources().contains_key(guid) {
                    return None;
                }

                let info = ResourceInfo {
                    resource_id: *guid,
                    original_value: OnceCell::new(),
                    value: RefCell::default(),
                    is_dirty: Cell::new(false),
                };
                let info = Rc::new(info);

                entry.insert(info.clone());

                Some(info)
            }
        }
    }

    pub fn add_resource(
        &self,
        value: &LuaValue,
        guid: &ResourceId,
        lua: &mlua::Lua,
    ) -> mlua::Result<()> {
        if !self.has_feature(AppFeatures::ASSETS_WRITE) {
            return Err(LuaError::generic("game.assets.write is unavailable"));
        }
        let mut resources = self.resources.borrow_mut();

        if resources.contains_key(guid) {
            return Err(LuaError::generic(format!(
                "resource with GUID {guid} already exists"
            )));
        }

        let r#type = TypeHandle::new(
            self.type_registry.clone(),
            kfc_format::type_for_resource(&self.type_registry, guid)
                .ok_or_else(|| {
                    LuaError::generic("resource type not found in current game registry")
                })?
                .index,
        );
        let value =
            validate_and_clone_lua_value(value, &r#type, lua).map_err(LuaError::external)?;

        let info = Rc::new(ResourceInfo {
            resource_id: *guid,
            original_value: OnceCell::new(),
            value: RefCell::new(Some(value)),
            is_dirty: Cell::new(true),
        });

        resources.insert(*guid, info);

        Ok(())
    }

    pub fn create_resource(
        &self,
        value: &LuaValue,
        type_index: TypeIndex,
        lua: &mlua::Lua,
    ) -> mlua::Result<ResourceId> {
        if !self.has_feature(AppFeatures::ASSETS_WRITE) {
            return Err(LuaError::generic("game.assets.write is unavailable"));
        }
        let resources = self.resources.borrow_mut();
        let mut guid: ResourceId;

        loop {
            let uuid = uuid::Uuid::new_v4().into_bytes();

            guid =
                kfc_format::resource_id_for_type(&self.type_registry, type_index, uuid.into(), 0)
                    .ok_or_else(|| {
                    LuaError::generic("resource type not found in current game registry")
                })?;

            if !resources.contains_key(&guid) {
                break;
            }
        }

        drop(resources);

        self.add_resource(value, &guid, lua)?;

        Ok(guid)
    }

    pub fn get_content(&self, guid: &ContentHash) -> mlua::Result<Option<Vec<u8>>> {
        if let Some(value) = self.new_contents.borrow().get(guid) {
            // OPTIMIZE: cloning here can be quite expensive
            Ok(Some(value.clone()))
        } else {
            // OPTIMIZE: consider using a reader for this
            Ok(self.reader.borrow_mut().read_content(guid)?)
        }
    }

    pub fn create_content(&self, data: &[u8]) -> mlua::Result<ContentHash> {
        if !self.has_feature(AppFeatures::ASSETS_WRITE) {
            return Err(LuaError::generic("game.assets.write is unavailable"));
        }
        let guid = ContentHash::from_data(data);

        if let hash_map::Entry::Vacant(e) = self.new_contents.borrow_mut().entry(guid) {
            self.writer
                .borrow_mut()
                .as_mut()
                .expect("KFCWriter no longer available")
                .write_content(&guid, data)?;

            // TODO: attach a reader to the writer to avoid cloning
            e.insert(data.to_vec());
        }

        Ok(guid)
    }

    pub fn get_type(
        &self,
        lua: &mlua::Lua,
        type_index: TypeIndex,
    ) -> mlua::Result<Option<LuaValue>> {
        match self.types.borrow_mut().entry(type_index) {
            Entry::Occupied(entry) => Ok(Some(entry.get().clone())),
            Entry::Vacant(entry) => {
                if self.type_registry.get(type_index).is_none() {
                    return Ok(None);
                }

                let value = Type::new(TypeHandle::new(self.type_registry.clone(), type_index));
                let value = LuaValue::UserData(lua.create_userdata(value)?);

                entry.insert(value.clone());
                Ok(Some(value))
            }
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if self.asset_transaction.get() {
            if let Err(error) = shroudforge_parser::transaction::abort(
                self.env.game_dir().as_std_path(),
                &self.asset_stem,
            ) {
                warn!(error = %error, "Failed to abort typed asset transaction");
            }
        }
    }
}

pub struct ResourceInfo {
    pub resource_id: ResourceId,

    original_value: OnceCell<Option<MappedValue>>,
    value: RefCell<Option<LuaValue>>,
    is_dirty: Cell<bool>,
}

impl ResourceInfo {
    pub fn get_lua_value(&self, lua: &mlua::Lua) -> mlua::Result<LuaValue> {
        if let Some(value) = self.value.borrow().as_ref() {
            return Ok(value.clone());
        }

        let value = match self.get_mapped_value(lua)? {
            Some(value) => convert_value_to_lua(value, lua)?,
            None => LuaValue::Nil,
        };

        self.value.replace(Some(value.clone()));

        Ok(value)
    }

    pub fn set_lua_value(&self, lua_value: LuaValue, lua: &mlua::Lua) -> mlua::Result<()> {
        let app_state = lua.app_data_ref::<AppState>().unwrap();
        if !app_state.has_feature(AppFeatures::ASSETS_WRITE) {
            return Err(LuaError::generic("game.assets.write is unavailable"));
        }
        let type_registry = app_state.type_registry();

        let r#type = kfc_format::type_for_resource(type_registry, &self.resource_id)
            .ok_or_else(|| LuaError::generic("resource type not found in current game registry"))?;

        let lua_value = validate_and_clone_lua_value(
            &lua_value,
            &TypeHandle::new(type_registry.clone(), r#type.index),
            lua,
        )
        .map_err(LuaError::external)?;

        self.value.replace(Some(lua_value));
        self.is_dirty.replace(true);

        Ok(())
    }

    pub fn apply(&self, lua: &mlua::Lua) -> mlua::Result<Option<Value>> {
        let app_state = lua.app_data_ref::<AppState>().unwrap();
        let type_registry = app_state.type_registry();

        let lua_value = self.value.borrow();
        let lua_value = match lua_value.as_ref() {
            Some(value) => value,
            None => return Ok(None),
        };

        if !self.is_dirty.get() && !is_dirty_lua_value(lua_value)? {
            return Ok(None);
        }

        let r#type = kfc_format::type_for_resource(type_registry, &self.resource_id)
            .ok_or_else(|| LuaError::generic("resource type not found in current game registry"))?;

        let value = convert_lua_to_value(
            lua_value,
            &TypeHandle::new(type_registry.clone(), r#type.index),
        )
        .map_err(LuaError::external)?;

        Ok(Some(value))
    }

    fn get_mapped_value(&self, lua: &mlua::Lua) -> mlua::Result<Option<&MappedValue>> {
        let app_state = lua.app_data_ref::<AppState>().unwrap();

        self.original_value
            .get_or_try_init(
                || match app_state.reader().read_resource(&self.resource_id)? {
                    Some(value) => {
                        let type_registry = app_state.type_registry();
                        let r#type =
                            kfc_format::type_for_resource(type_registry, &self.resource_id)
                                .ok_or_else(|| {
                                    LuaError::generic(
                                        "resource type not found in current game registry",
                                    )
                                })?;

                        let value = MappedValue::from_bytes(
                            type_registry,
                            r#type,
                            &Rc::from(value.into_boxed_slice()),
                        )
                        .map_err(LuaError::external)?;

                        Ok(Some(value))
                    }
                    None => Ok(None),
                },
            )
            .map(|v| v.as_ref())
    }
}
