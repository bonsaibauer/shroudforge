use std::{
    cell::{Cell, RefCell, RefMut},
    collections::{
        HashMap, HashSet,
        hash_map::{self, Entry},
    },
    rc::Rc,
};

use bitflags::bitflags;
use kfc::kfc_format;
use kfc::{
    container::{KFCCursor, KFCFile, KFCReader, KFCWriter},
    guid::{ContentHash, ResourceId},
    reflection::{TypeHandle, TypeIndex, TypeRegistry},
    resource::value::Value,
};
use mod_loader::{Mod, ModEnvironment};
use once_cell::unsync::OnceCell;

use crate::{
    RunArgs, RuntimePhase, ShroudForgeApi,
    cache::CacheDiff,
    env::{
        Type,
        game::value::is_dirty_lua_value,
        value::{convert_lua_to_value, convert_value_to_lua, validate_and_clone_lua_value},
    },
    logging::{error, warn},
    lua::{LuaError, LuaValue},
    type_aliases::{MappedValue, PathBuf},
};

pub(crate) const GAME_FILE_HANDLES_RELEASED: &str =
    "game asset files were released for the asset transaction";

pub struct AppState {
    pub(crate) runtime_configured: Cell<bool>,
    runtime_active_mods: RefCell<HashSet<String>>,
    runtime_effects: RefCell<HashMap<String, serde_json::Value>>,
    runtime_write_fault: RefCell<Option<(u64, String)>>,
    runtime_ownership:
        Rc<RefCell<crate::lua::ingame::overrides::claim_runtime_storage::OwnershipStore>>,
    env: ModEnvironment,
    api: ShroudForgeApi,
    config: AppConfig,

    type_registry: Rc<TypeRegistry>,
    attribute_catalog: OnceCell<Result<crate::lua::ingame::ecs::attributes::Catalog, String>>,
    inline_fields:
        RefCell<HashMap<TypeIndex, HashMap<String, Rc<crate::lua::values::InlineField>>>>,
    inline_field_bytes: Cell<usize>,

    ref_file: Rc<KFCFile>,
    /// Open handles on the game's resource and container files. Released
    /// before the asset transaction publishes or rolls back, because Wine
    /// refuses to replace a file name while any handle on it is open.
    reader: RefCell<Option<KFCCursor<KFCReader>>>,
    writer: RefCell<Option<KFCWriter<Rc<KFCFile>, Rc<TypeRegistry>>>>,
    asset_transaction: Cell<bool>,
    asset_stem: String,

    // NOTE: the `Vec<u8>` must always be treated as immutable
    new_contents: RefCell<HashMap<ContentHash, Vec<u8>>>,
    resources: RefCell<HashMap<ResourceId, Rc<ResourceInfo>>>,
    loaded_resource_indexes: RefCell<loaded_resource_indexes::Indexes>,
    types: RefCell<HashMap<TypeIndex, LuaValue>>,
}

pub struct AppConfig {
    is_server: bool,
    phase: RuntimePhase,

    feature_flags: AppFeatures,
    export_dir: PathBuf,
}

bitflags! {
    pub struct AppFeatures: u32 {
        const PATCH = 1 << 0;
        const EXPORT = 1 << 1;
        const RUNTIME_DLL = 1 << 2;

        const ALL = !0;
    }
}

impl AppState {
    #[inline]
    pub fn env(&self) -> &ModEnvironment {
        &self.env
    }

    #[inline]
    pub fn api(&self) -> &ShroudForgeApi {
        &self.api
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
    pub fn type_registry(&self) -> &Rc<TypeRegistry> {
        &self.type_registry
    }

    pub(crate) fn attribute_catalog(
        &self,
    ) -> Result<&crate::lua::ingame::ecs::attributes::Catalog, String> {
        self.attribute_catalog
            .get_or_init(|| match self.reader.borrow_mut().as_mut() {
                Some(reader) => crate::lua::ingame::ecs::attributes::Catalog::load(
                    &self.type_registry,
                    &self.ref_file,
                    reader,
                ),
                None => Err(GAME_FILE_HANDLES_RELEASED.to_owned()),
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(crate) fn inline_field(
        &self,
        root: TypeIndex,
        path: &str,
    ) -> Result<Rc<crate::lua::values::InlineField>, String> {
        if let Some(plan) = self
            .inline_fields
            .borrow()
            .get(&root)
            .and_then(|fields| fields.get(path))
        {
            return Ok(plan.clone());
        }
        let plan = Rc::new(crate::lua::values::inline_field(
            &self.type_registry,
            root,
            path,
        )?);
        let mut fields = self.inline_fields.borrow_mut();
        // Bound metadata retention. A full cache never discards a live value,
        // baseline or claim and does not turn a valid field into an error.
        let retained = plan
            .retained_bytes()
            .saturating_add(path.len())
            .saturating_add(128);
        let field_count: usize = fields.values().map(HashMap::len).sum();
        if field_count < 4096
            && self.inline_field_bytes.get().saturating_add(retained) <= 16 * 1024 * 1024
        {
            self.inline_field_bytes
                .set(self.inline_field_bytes.get() + retained);
            fields
                .entry(root)
                .or_default()
                .insert(path.to_owned(), plan.clone());
        }
        Ok(plan)
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
            // Struct fields drop only after this body, so the handles must be
            // closed explicitly before the rollback replaces any file.
            self.reader.get_mut().take();
            self.writer.get_mut().take();
            if let Err(error) = shroudforge_game_assets::transaction::abort(
                self.env.game_dir().as_std_path(),
                &self.asset_stem,
            ) {
                warn!(error = %error, "Failed to abort typed asset transaction");
            }
        }
    }
}

mod asset_resources;
pub(crate) mod configure_component_contract;
mod create_mod_session;
pub(crate) mod loaded_resource_indexes;
mod loaded_resources;
mod native_plugins;
mod record_runtime_state;
pub use asset_resources::ResourceInfo;
use asset_resources::ResourceSource;
