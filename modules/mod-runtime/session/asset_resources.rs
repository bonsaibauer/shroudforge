use super::*;

pub struct ResourceInfo {
    pub resource_id: ResourceId,
    pub(super) source: ResourceSource,
    pub(super) creation_session: u64,
    pub(super) runtime_snapshot: OnceCell<Rc<[u8]>>,
    pub(super) runtime_write_fault: RefCell<Option<String>>,

    pub(super) original_value: OnceCell<Option<MappedValue>>,
    pub(super) value: RefCell<Option<LuaValue>>,
    pub(super) is_dirty: Cell<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ResourceSource {
    KfcFile,
    AssetTransaction,
    LoadedRuntime,
}

impl ResourceInfo {
    pub fn source_name(&self) -> &'static str {
        match self.source {
            ResourceSource::KfcFile => "kfc-file",
            ResourceSource::AssetTransaction => "asset-transaction",
            ResourceSource::LoadedRuntime => "loaded-runtime",
        }
    }

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
        if self.source == ResourceSource::LoadedRuntime {
            return Err(LuaError::generic("loaded runtime resources are read-only"));
        }
        let app_state = lua.app_data_ref::<AppState>().unwrap();
        if !app_state.has_feature(AppFeatures::PATCH) {
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
        if self.source == ResourceSource::LoadedRuntime {
            return Ok(None);
        }
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
            .get_or_try_init(|| {
                let bytes = match self.source {
                    ResourceSource::KfcFile => app_state
                        .reader()?
                        .read_resource(&self.resource_id)?
                        .map(|bytes| Rc::<[u8]>::from(bytes)),
                    ResourceSource::AssetTransaction => None,
                    ResourceSource::LoadedRuntime => Some(
                        self.loaded_snapshot_bytes()
                            .map_err(LuaError::generic)?
                            .clone(),
                    ),
                };
                match bytes {
                    Some(value) => {
                        let type_registry = app_state.type_registry();
                        let r#type =
                            kfc_format::type_for_resource(type_registry, &self.resource_id)
                                .ok_or_else(|| {
                                    LuaError::generic(
                                        "resource type not found in current game registry",
                                    )
                                })?;

                        let value = MappedValue::from_bytes(type_registry, r#type, &value)
                            .map_err(LuaError::external)?;

                        Ok(Some(value))
                    }
                    None => Ok(None),
                }
            })
            .map(|v| v.as_ref())
    }
}

impl AppState {
    #[inline]
    pub fn kfc_file(&self) -> &KFCFile {
        &self.ref_file
    }

    #[inline]
    pub fn reader(&self) -> mlua::Result<RefMut<'_, KFCCursor<KFCReader>>> {
        RefMut::filter_map(self.reader.borrow_mut(), Option::as_mut)
            .map_err(|_| LuaError::generic(super::GAME_FILE_HANDLES_RELEASED))
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
        // Publishing replaces enshrouded.kfc_resources, which the reader keeps
        // open. Windows replaces open files with POSIX semantics, Wine/Proton
        // rejects that with "Access denied (os error 5)".
        self.reader.borrow_mut().take();
        shroudforge_game_assets::transaction::commit(
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
                    source: ResourceSource::KfcFile,
                    creation_session: 0,
                    runtime_snapshot: OnceCell::new(),
                    runtime_write_fault: RefCell::new(None),
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
        if !self.has_feature(AppFeatures::PATCH) {
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
            source: ResourceSource::AssetTransaction,
            creation_session: 0,
            runtime_snapshot: OnceCell::new(),
            runtime_write_fault: RefCell::new(None),
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
        if !self.has_feature(AppFeatures::PATCH) {
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
            Ok(self.reader()?.read_content(guid)?)
        }
    }

    pub fn create_content(&self, data: &[u8]) -> mlua::Result<ContentHash> {
        if !self.has_feature(AppFeatures::PATCH) {
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
}
