use mlua::{Function, RegistryKey, Value};
use std::path::PathBuf;
use std::sync::Arc;

use mod_loader::ModEnvironment;
use shroudforge_compatibility::GameContract;
use shroudforge_parser::{ResourceReference, TypeDefinition};

use crate::{
    alias::Path,
    cache::{CacheDiff, FileStateCache},
    env::{AppFeatures, AppState},
    log::info,
    runner::LuaModRunner,
};

mod alias;
mod cache;
mod definition;
mod env;
mod load;
mod log;
mod lua;
mod runner;
mod util;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parsed and compatibility-verified game model used by the Lua API.
#[derive(Clone)]
pub struct ShroudForgeApi {
    contract: Arc<GameContract>,
}

impl ShroudForgeApi {
    pub fn new(contract: GameContract) -> Self {
        Self {
            contract: Arc::new(contract),
        }
    }

    pub fn game_version(&self) -> &str {
        &self.contract.schema().game_version
    }

    pub fn parser_id(&self) -> &str {
        &self.contract.schema().parser
    }

    pub fn type_by_name(&self, name: &str) -> Option<&TypeDefinition> {
        self.contract.schema().types.get(name)
    }

    pub fn types(&self) -> impl Iterator<Item = &TypeDefinition> {
        self.contract.schema().types.values()
    }

    pub fn resource_types(&self) -> impl Iterator<Item = (&str, usize)> {
        self.contract
            .schema()
            .resources
            .iter()
            .map(|(name, values)| (name.as_str(), values.len()))
    }

    pub fn resources(&self, type_name: &str) -> &[ResourceReference] {
        self.contract
            .schema()
            .resources
            .get(type_name)
            .map_or(&[], Vec::as_slice)
    }

    pub fn has_runtime(&self, operation: &str) -> bool {
        self.contract.has_runtime(operation)
    }

    pub fn runtime(&self, operation: &str) -> Availability {
        self.contract.runtime(operation)
    }
}

pub use shroudforge_compatibility::Availability;
pub use shroudforge_parser::{
    EnumValueDefinition, FieldDefinition, GameFiles, GameParser, KfcParser, ParserError,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimePhase {
    #[default]
    Pregame,
    Ingame,
}

impl RuntimePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pregame => "pregame",
            Self::Ingame => "ingame",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// If true, it will ignore the cache and re-apply all mods.
    pub skip_cache: bool,
    /// If None, it will auto-detect based on the files within the game directory.
    pub is_server: Option<bool>,
    /// If true, mods may write typed resources back to the game asset container.
    pub assets_write: bool,
    /// If true, it will allow mods to use `io.export` to export files to the export directory.
    pub export: bool,
    /// If None, it will use the default export directory (`<game_dir>/export`).
    pub export_dir: Option<PathBuf>,
    /// Execution phase. This changes availability, never the public API shape.
    pub phase: RuntimePhase,
}

#[derive(Debug, Clone)]
pub struct RunArgs {
    pub file_name: String,
    /// The features to enable.
    pub options: RunOptions,
}

pub fn run(env: &ModEnvironment, api: ShroudForgeApi, mut args: RunArgs) -> anyhow::Result<()> {
    info!("Running lua with options: {:?}", args);

    // check cache if files have changed

    let cache_diff = if !args.options.skip_cache {
        let current_cache = FileStateCache::read(env.cache_dir());
        let mut new_cache = FileStateCache::default();

        new_cache.track_game_files(env.game_dir());
        new_cache.track_mod_files(env.mods_dir());
        new_cache.track(env.game_dir().join("config").join("shroudforge.json"));

        let cache_diff = new_cache.diff(&current_cache);

        if cache_diff.is_none() && args.options.assets_write {
            args.options.assets_write = false;

            info!("No changes detected, skipping asset writes");
        }

        cache_diff
    } else {
        info!("Skipping cache check");
        CacheDiff::new_dirty()
    };

    // create a new app state

    let app_state = match AppState::new(env.clone(), api, args, &cache_diff) {
        Ok(context) => context,
        Err(_) => anyhow::bail!("Failed to create AppState"),
    };

    // setup and run all mods

    let runner = LuaModRunner::new(app_state)?;

    match runner.setup(env.enabled_mods()) {
        Ok(_) => {}
        Err(err) => {
            anyhow::bail!("Failed to setup LuaModRunner: {}", err);
        }
    }

    info!("Running mods...");

    // TODO: move this somewhere else, maybe to the GameContext?

    runner.run()?;

    let app_state = runner.lua.app_data_ref::<AppState>().unwrap();

    if app_state.has_feature(AppFeatures::ASSETS_WRITE) {
        info!("Committing typed asset changes...");

        let mut buf = Vec::new();
        let mut writer = app_state.take_writer()?;
        let resources = app_state.get_cached_resources();
        let mut applied_resources = 0;

        for resource in resources.iter() {
            let value = resource.apply(&runner.lua)?;

            if let Some(value) = value {
                let r#type = shroudforge_parser::kfc_format::type_for_resource(
                    app_state.type_registry(),
                    &resource.resource_id,
                )
                .expect("resource type must exist in current game registry");

                buf.clear();
                value
                    .write_into(app_state.type_registry(), r#type, &mut buf)
                    .unwrap();
                writer.write_resource(&resource.resource_id, &buf)?;
                applied_resources += 1;
            }
        }

        writer.finalize()?;
        app_state.commit_assets()?;

        info!("Committed {} changed resources", applied_resources);
    }

    // create a new cache file

    if app_state.has_feature(AppFeatures::ASSETS_WRITE) {
        let mut new_cache = FileStateCache::default();

        new_cache.track_game_files(env.game_dir());
        new_cache.track_mod_files(env.mods_dir());
        new_cache.track(env.game_dir().join("config").join("shroudforge.json"));

        new_cache.write(env.cache_dir());
    } else {
        let mut new_cache = FileStateCache::read(env.cache_dir());

        new_cache.track_game_files(env.game_dir());
        new_cache.track(env.game_dir().join("config").join("shroudforge.json"));
        new_cache.write(env.cache_dir());
    }

    Ok(())
}

struct Lifecycle {
    id: String,
    update: Option<RegistryKey>,
    unload: Option<RegistryKey>,
    active: bool,
}

/// Long-lived in-game execution of the same ShroudForge Lua API used pregame.
pub struct IngameRuntime {
    runner: LuaModRunner,
    lifecycle: Vec<Lifecycle>,
}

impl IngameRuntime {
    pub fn start(
        env: &ModEnvironment,
        api: ShroudForgeApi,
        file_name: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let state = AppState::new(
            env.clone(),
            api,
            RunArgs {
                file_name: file_name.into(),
                options: RunOptions {
                    skip_cache: false,
                    is_server: None,
                    assets_write: false,
                    export: true,
                    export_dir: None,
                    phase: RuntimePhase::Ingame,
                },
            },
            &CacheDiff::new_dirty(),
        )
        .map_err(|_| anyhow::anyhow!("failed to initialize ShroudForge API"))?;
        let runner = LuaModRunner::new(state)?;
        runner.setup(env.enabled_mods())?;
        let mut lifecycle = Vec::new();
        for id in runner.runtime_mod_ids() {
            let value = match runner.load_runtime_module(&id) {
                Ok(value) => value,
                Err(error) => {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                        "module initialization failed; scope discarded: {error}");
                    continue;
                }
            };
            let table = match value {
                Value::Table(table) => Some(table),
                Value::Nil => None,
                _ => {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                        "entrypoint must return a lifecycle table or nil");
                    continue;
                }
            };
            let (load, update, unload) = match (
                callback(&runner.lua, table.as_ref(), "on_load"),
                callback(&runner.lua, table.as_ref(), "on_update"),
                callback(&runner.lua, table.as_ref(), "on_unload"),
            ) {
                (Ok(load), Ok(update), Ok(unload)) => (load, update, unload),
                _ => {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                        "invalid lifecycle; scope discarded");
                    continue;
                }
            };
            if let Some(key) = load {
                if let Err(error) = runner.lua.registry_value::<Function>(&key)?.call::<()>(()) {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                        "on_load failed; rolling back scope: {error}");
                    if let Some(key) = &unload {
                        if let Err(cleanup) = runner
                            .lua
                            .registry_value::<Function>(key)
                            .and_then(|callback| callback.call::<()>(()))
                        {
                            tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                                "rollback failed; recovery required: {cleanup}");
                        }
                    }
                    continue;
                }
            }
            lifecycle.push(Lifecycle {
                id,
                update,
                unload,
                active: true,
            });
        }
        Ok(Self { runner, lifecycle })
    }

    pub fn update(&mut self, delta_seconds: f64) {
        crate::env::shroudforge::dispatch_ui_actions(&self.runner.lua);
        for item in &mut self.lifecycle {
            if !item.active {
                continue;
            }
            if let Some(key) = &item.update {
                if let Err(error) = self
                    .runner
                    .lua
                    .registry_value::<Function>(key)
                    .and_then(|callback| callback.call::<()>(delta_seconds))
                {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %item.id,
                        "on_update failed: {error}");
                    if let Some(unload) = &item.unload {
                        if let Err(cleanup) = self
                            .runner
                            .lua
                            .registry_value::<Function>(unload)
                            .and_then(|callback| callback.call::<()>(()))
                        {
                            tracing::error!(target: "shroudforge::runtime", mod_id = %item.id,
                                "update rollback failed; recovery required: {cleanup}");
                        }
                    }
                    item.active = false;
                }
            }
        }
    }
}

impl Drop for IngameRuntime {
    fn drop(&mut self) {
        for item in self.lifecycle.iter().rev() {
            if !item.active {
                continue;
            }
            if let Some(key) = &item.unload {
                if let Err(error) = self
                    .runner
                    .lua
                    .registry_value::<Function>(key)
                    .and_then(|callback| callback.call::<()>(()))
                {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %item.id,
                        "on_unload failed: {error}");
                }
            }
        }
    }
}

fn callback(
    lua: &mlua::Lua,
    lifecycle: Option<&mlua::Table>,
    name: &str,
) -> mlua::Result<Option<RegistryKey>> {
    let value = if let Some(table) = lifecycle {
        table.get::<Value>(name)?
    } else {
        Value::Nil
    };
    match value {
        Value::Nil => Ok(None),
        Value::Function(value) => Ok(Some(lua.create_registry_value(value)?)),
        _ => Err(mlua::Error::runtime(format!("{name} must be a function"))),
    }
}

pub fn export_lua_definitions(game_dir: impl AsRef<Path>, file_name: &str, force: bool) -> bool {
    let game_dir = game_dir.as_ref();
    let cache_dir = game_dir.join(".cache");
    let (type_registry, is_dirty) =
        match crate::load::load_type_registry(game_dir, &cache_dir, file_name) {
            Ok(type_registry) => type_registry,
            Err(_) => return false,
        };

    crate::load::export_lua_definitions(&cache_dir, &type_registry, is_dirty || force);

    true
}

pub fn restore(game_dir: impl AsRef<Path>, file_name: &str) -> bool {
    let kfc_path = game_dir.as_ref().join(format!("{file_name}.kfc"));

    load::restore_backup(&kfc_path).is_ok()
}
