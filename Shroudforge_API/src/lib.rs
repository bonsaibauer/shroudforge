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

/// Deployed contract must describe this binary, not invent native capabilities.
/// ECS readiness and per-component compatibility are still checked at call time.
pub fn runtime_operations(root: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let embedded: serde_json::Value = serde_json::from_str(include_str!("../../config/api/api.json"))?;
    let path = mod_loader::config::document_path(root, "api");
    let value = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => embedded.clone(),
        Err(error) => return Err(error.into()),
    };
    mod_loader::config::validate_document(root, "api", &value).map_err(anyhow::Error::msg)?;
    if value["apiVersion"] != API_VERSION
        || value["runtimeProviderAbi"] != embedded["runtimeProviderAbi"]
        || value["runtimeOperations"] != embedded["runtimeOperations"]
    {
        anyhow::bail!("config/api.json does not match the installed API binary");
    }
    Ok(serde_json::from_value(value["runtimeOperations"].clone())?)
}

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
    /// If true, keep the asset writer enabled even when input fingerprints match.
    /// Used when a caller has explicitly restored the clean KFC baseline.
    pub force_assets: bool,
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

pub fn run(env: &ModEnvironment, api: ShroudForgeApi, args: RunArgs) -> anyhow::Result<()> {
    run_with_api(env, Some(api), args)
}

/// Run a bounded startup/pregame asset pass using the installed runtime
/// compatibility profile and schema inferred from the local game files.
pub fn run_with_local_schema(env: &ModEnvironment, args: RunArgs) -> anyhow::Result<()> {
    run_with_api(env, None, args)
}

fn run_with_api(env: &ModEnvironment, api: Option<ShroudForgeApi>, mut args: RunArgs) -> anyhow::Result<()> {
    info!("Running lua with options: {:?}", args);

    // check cache if files have changed

    let cache_diff = if !args.options.skip_cache {
        let current_cache = FileStateCache::read(env.cache_dir());
        let mut new_cache = FileStateCache::default();

        new_cache.track_game_files(env.game_dir());
        new_cache.track_mod_files(env.mods_dir());
        new_cache.track_mod_config(env.game_dir());

        let cache_diff = new_cache.diff(&current_cache);

        if cache_diff.is_none() && args.options.assets_write && !args.options.force_assets {
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

    let is_server = runner.lua.app_data_ref::<AppState>().unwrap().is_server();
    match runner.setup(env.plan(is_server, API_VERSION)) {
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
        new_cache.track_mod_config(env.game_dir());

        new_cache.write(env.cache_dir());
    } else {
        let mut new_cache = FileStateCache::read(env.cache_dir());

        new_cache.track_game_files(env.game_dir());
        new_cache.track_mod_config(env.game_dir());
        new_cache.write(env.cache_dir());
    }

    Ok(())
}

struct Lifecycle {
    id: String,
    load: Option<RegistryKey>,
    update: Option<RegistryKey>,
    unload: Option<RegistryKey>,
    update_interval: std::time::Duration,
    next_update: std::time::Instant,
    active: bool,
}

/// Long-lived in-game execution of the same ShroudForge Lua API used pregame.
pub struct IngameRuntime {
    diagnostics: shroudforge_runtime_diagnostics::Session,
    root: std::path::PathBuf,
    loaded: serde_json::Map<String,serde_json::Value>,
    errors: serde_json::Map<String,serde_json::Value>,
    next_status: std::time::Instant,
    configurations: Vec<(String, std::path::PathBuf, serde_json::Value)>,
    observed: std::collections::HashMap<std::path::PathBuf, String>,
    runner: LuaModRunner,
    lifecycle: Vec<Lifecycle>,
}

impl IngameRuntime {
    pub fn active_mod_ids(&self) -> Vec<String> {
        self.lifecycle
            .iter()
            .filter(|item| item.active)
            .map(|item| item.id.clone())
            .collect()
    }

    pub fn start(
        env: &ModEnvironment,
        api: ShroudForgeApi,
        file_name: impl Into<String>,
    ) -> anyhow::Result<Self> {
        Self::start_with_schema(env, Some(api), file_name.into())
    }

    pub fn start_live(env: &ModEnvironment, file_name: impl Into<String>) -> anyhow::Result<Self> {
        Self::start_with_schema(env, None, file_name.into())
    }

    fn start_with_schema(
        env: &ModEnvironment,
        api: Option<ShroudForgeApi>,
        file_name: String,
    ) -> anyhow::Result<Self> {
        let mut diagnostics=shroudforge_runtime_diagnostics::Session::new(env.game_dir().as_std_path());
        let state = AppState::new(
            env.clone(),
            api,
            RunArgs {
                file_name: file_name.into(),
                options: RunOptions {
                    skip_cache: false,
                    force_assets: false,
                    is_server: None,
                    assets_write: false,
                    export: true,
                    export_dir: None,
                    phase: RuntimePhase::Ingame,
                },
            },
            &CacheDiff::new_dirty(),
        );
        let state=state.map_err(|_| anyhow::anyhow!("failed to initialize ShroudForge API"))?;
        let runner = LuaModRunner::new(state)?;
        let is_server = runner.lua.app_data_ref::<AppState>().unwrap().is_server();
        let root = env.game_dir().as_std_path().to_path_buf();
        let (mut plan, plan_errors) = env.runtime_plan_report(is_server, API_VERSION);
        let mut errors = serde_json::Map::new();
        for (index, error) in plan_errors.into_iter().enumerate() { errors.insert(format!("plan-{index}"), error.into()); }
        let needs_assets = plan.iter().any(|item| item.info().enabled && item.info().capabilities.contains(&mod_loader::Capability::AssetsWrite));
        if needs_assets {
            let ready = mod_loader::prepared::fingerprint(env,is_server,API_VERSION).map(|fingerprint|mod_loader::prepared::matches(&root,&fingerprint)).unwrap_or(false);
            if !ready {
                let mut blocked: std::collections::HashSet<String> = plan.iter().filter(|item|item.info().enabled && item.info().capabilities.contains(&mod_loader::Capability::AssetsWrite)).map(|item|item.info().id.clone()).collect();
                loop {
                    let before=blocked.len();
                    for item in &plan { if item.info().dependencies.iter().any(|dependency|!dependency.optional.unwrap_or(false)&&blocked.contains(&dependency.id)) {blocked.insert(item.info().id.clone());} }
                    if before==blocked.len(){break;}
                }
                plan.retain(|item| {
                    if blocked.contains(&item.info().id) { errors.insert(item.info().id.clone(), "startup-asset-apply-required: the early startup transaction did not publish this asset configuration".into()); false } else {true}
                });
            }
        }
        let loaded=plan.iter().map(|item| {
            let bytes=serde_json::to_vec(item.info()).unwrap_or_default();
            (item.info().id.clone(),serde_json::json!(mod_loader::config::revision(&bytes)))
        }).collect();
        runner.setup(plan)?;
        let mut lifecycle = Vec::new();
        for id in runner.runtime_mod_ids() {
            let module_result=runner.load_runtime_module(&id);
            let value = match module_result {
                Ok(value) => value,
                Err(error) => {
                    errors.insert(id.clone(),format!("module initialization failed: {error}").into());
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
                    errors.insert(id.clone(),"entrypoint must return a lifecycle table or nil".into());
                    continue;
                }
            };
            let update_interval_ms = match table.as_ref() {
                Some(table) => table.get::<Option<u64>>("update_interval_ms"),
                None => Ok(None),
            };
            let update_interval_ms = match update_interval_ms {
                Ok(Some(value)) if (8..=1000).contains(&value) => value,
                Ok(None) => 50,
                Ok(Some(_)) | Err(_) => {
                    errors.insert(id.clone(), "update_interval_ms must be an integer from 8 to 1000".into());
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
                    errors.insert(id.clone(),"invalid lifecycle".into());
                    continue;
                }
            };
            let enabled = env.mod_registry().get(&id).is_some_and(|item| item.info().enabled);
            if enabled {
                runner.lua.app_data_ref::<AppState>().unwrap().set_runtime_mod_active(&id, true);
            }
            if enabled && let Some(key) = load.as_ref() {
                let started=std::time::Instant::now();
                let result=runner.lua.registry_value::<Function>(&key)?.call::<()>(());
                diagnostics.measure("mods",&id,"on_load",started.elapsed(),result.is_err());
                if let Err(error) = result {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %id,
                        "on_load failed; rolling back scope: {error}");
                    errors.insert(id.clone(),format!("on_load failed: {error}").into());
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
                    runner.lua.app_data_ref::<AppState>().unwrap().set_runtime_mod_active(&id, false);
                    continue;
                }
            }
            lifecycle.push(Lifecycle {
                id,
                load,
                update,
                unload,
                update_interval: std::time::Duration::from_millis(update_interval_ms),
                next_update: std::time::Instant::now(),
                active: enabled,
            });
        }
        let configurations = env.mod_registry().values().map(|item| (item.info().id.clone(), item.fs().root().as_std_path().to_path_buf(), serde_json::to_value(item.info()).expect("manifest is serializable"))).collect();
        let runtime=Self { diagnostics, runner, lifecycle, root, loaded, errors, configurations, observed: Default::default(), next_status:std::time::Instant::now() };
        runtime.publish_status(true);
        Ok(runtime)
    }

    fn publish_status(&self, running: bool) {
        let value=serde_json::json!({
            "schemaVersion":1,"pid":std::process::id(),"apiVersion":API_VERSION,
            "updatedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            "running":running,"active":if running {self.active_mod_ids()}else{Vec::new()},
            "loaded":self.loaded,"errors":self.errors,
            "runtimeProvider":crate::env::runtime_provider_report()
        });
        if let Err(error)=mod_loader::config::write_document(&self.root,"mod-status",&value){tracing::error!(target:"shroudforge::runtime","status publication failed: {error}");}
    }

    pub fn update(&mut self, delta_seconds: f64) {
        self.diagnostics.tick();
        if std::time::Instant::now() >= self.next_status {
            self.refresh_configuration();
            self.publish_status(true);
            self.next_status=std::time::Instant::now()+std::time::Duration::from_secs(1);
        }
        crate::env::shroudforge::dispatch_ui_actions(&self.runner.lua, &self.active_mod_ids());
        for item in &mut self.lifecycle {
            if !item.active {
                continue;
            }
            if let Some(key) = &item.update {
                if std::time::Instant::now() < item.next_update { continue; }
                let started=self.diagnostics.enabled("mods").then(std::time::Instant::now);
                let result = self
                    .runner
                    .lua
                    .registry_value::<Function>(key)
                    .and_then(|callback| callback.call::<()>(delta_seconds));
                if let Some(started)=started {self.diagnostics.measure("mods",&item.id,"on_update",started.elapsed(),result.is_err());}
                item.next_update = std::time::Instant::now() + item.update_interval;
                if let Err(error) = result {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %item.id,
                        "on_update failed: {error}");
                    self.errors.insert(item.id.clone(),format!("on_update failed: {error}").into());
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
                    self.runner.lua.app_data_ref::<AppState>().unwrap().set_runtime_mod_active(&item.id, false);
                    item.active = false;
                }
            }
        }
    }

    fn refresh_configuration(&mut self) {
        let configured_enabled: std::collections::HashMap<String, bool> = self.configurations.iter()
            .map(|(id, _, configuration)| (id.clone(), configuration["enabled"].as_bool().unwrap_or(false)))
            .collect();
        for (id, package, applied) in &mut self.configurations {
            // Legacy ZIP packages remain supported at startup but are not mutable live.
            if !package.is_dir() { continue; }
            let path = package.join("mod.json");
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    let signature = format!("read-error:{error}");
                    if self.observed.get(&path) != Some(&signature) {
                        tracing::error!(target:"shroudforge::runtime", mod_id=%id, "Configuration read failed; keeping applied values: {error}");
                        self.observed.insert(path, signature);
                    }
                    continue;
                }
            };
            let signature = mod_loader::config::revision(&bytes);
            if self.observed.get(&path) == Some(&signature) { continue; }
            self.observed.insert(path, signature);
            let parsed = mod_loader::config::read_manifest_path(&self.root, package);
            let manifest = match parsed {
                Ok(manifest) if manifest.id == *id => manifest,
                Ok(_) => { tracing::error!(target:"shroudforge::runtime", mod_id=%id,"Mod identity changed; restart required, retaining applied configuration"); continue; }
                Err(error) => { tracing::error!(target:"shroudforge::runtime", mod_id=%id,"Configuration rejected; keeping applied values: {error}"); continue; }
            };
            let desired = serde_json::to_value(&manifest).expect("manifest is serializable");
            if desired == *applied { continue; }
            let mut metadata = desired.clone();
            metadata["settingValues"] = applied["settingValues"].clone();
            metadata["enabled"] = applied["enabled"].clone();
            let runtime_mod = manifest.capabilities.contains(&mod_loader::Capability::Runtime);
            let live_mod = runtime_mod && !manifest.capabilities.contains(&mod_loader::Capability::AssetsWrite);
            let metadata_unchanged = metadata == *applied;
            let wanted_enabled = manifest.enabled;
            let was_enabled = applied["enabled"].as_bool().unwrap_or(false);
            if metadata_unchanged && live_mod && wanted_enabled != was_enabled {
                let missing_dependency = wanted_enabled.then(|| manifest.dependencies.iter().find(|dependency| {
                    if dependency.optional.unwrap_or(false) { return false; }
                    let config_disabled=configured_enabled.get(&dependency.id).is_some_and(|enabled| !enabled);
                    let runtime_disabled=self.lifecycle.iter().any(|candidate|candidate.id==dependency.id && !candidate.active);
                    config_disabled || runtime_disabled
                }).map(|dependency| dependency.id.clone())).flatten();
                if let Some(dependency) = missing_dependency {
                    let error = format!("required dependency is not active: {dependency}");
                    self.errors.insert(id.clone(), error.clone().into());
                    tracing::error!(target:"shroudforge::runtime", mod_id=%id, "Live activation transition failed: {error}");
                } else if let Some(item) = self.lifecycle.iter_mut().find(|item| item.id == *id) {
                    let transition = if wanted_enabled {
                        self.runner.lua.app_data_ref::<AppState>().unwrap().set_runtime_mod_active(id, true);
                        if let Some(key) = &item.load {
                            self.runner.lua.registry_value::<Function>(key).and_then(|callback| callback.call::<()>(())).map_err(|error| error.to_string())
                        } else { Ok(()) }
                    } else if let Some(key) = &item.unload {
                        self.runner.lua.registry_value::<Function>(key).and_then(|callback| callback.call::<()>(())).map_err(|error| error.to_string())
                    } else { Ok(()) };
                    match transition {
                        Ok(()) => {
                            item.active = wanted_enabled;
                            if !wanted_enabled {
                                self.runner.lua.app_data_ref::<AppState>().unwrap().set_runtime_mod_active(id, false);
                            }
                            applied["enabled"] = serde_json::json!(wanted_enabled);
                            self.errors.remove(id);
                            tracing::info!(target:"shroudforge::runtime", mod_id=%id, enabled=wanted_enabled, "Runtime mod activation changed live");
                        }
                        Err(error) => {
                            if wanted_enabled {
                                if let Some(key) = &item.unload {
                                    if let Err(rollback) = self.runner.lua.registry_value::<Function>(key)
                                        .and_then(|callback| callback.call::<()>(())) {
                                        tracing::error!(target:"shroudforge::runtime", mod_id=%id, "Failed live activation rollback: {rollback}");
                                    }
                                }
                                self.runner.lua.app_data_ref::<AppState>().unwrap().set_runtime_mod_active(id, false);
                            }
                            self.errors.insert(id.clone(), format!("live activation failed: {error}").into());
                            tracing::error!(target:"shroudforge::runtime", mod_id=%id, "Live activation transition failed: {error}");
                        }
                    }
                } else {
                    self.errors.insert(id.clone(), "runtime module was not present in the startup plan; restart required".into());
                }
            }
            let desired_settings = manifest.setting_values.clone();
            let settings_changed = desired_settings != applied["settingValues"];
            if metadata_unchanged && live_mod && settings_changed {
                match crate::env::shroudforge::set_settings(&self.runner.lua,id,&desired_settings) {
                    Ok(()) => {
                        applied["settingValues"] = desired_settings;
                        tracing::info!(target:"shroudforge::runtime", mod_id=%id,"Runtime settings applied live");
                    }
                    Err(error) => {
                        self.errors.insert(id.clone(), format!("live settings update failed: {error}").into());
                        tracing::error!(target:"shroudforge::runtime", mod_id=%id,"Live settings update failed: {error}");
                    }
                }
            }
            let now_applied = serde_json::to_value(&manifest).expect("manifest is serializable");
            let changed_but_deferred = now_applied != *applied;
            let deferred_is_only_enable_or_settings = {
                let mut comparable = now_applied.clone();
                comparable["enabled"] = applied["enabled"].clone();
                comparable["settingValues"] = applied["settingValues"].clone();
                comparable == *applied
            };
            if changed_but_deferred && !deferred_is_only_enable_or_settings {
                tracing::info!(target:"shroudforge::runtime", mod_id=%id, "Mod changes require the next game start");
            } else if changed_but_deferred && !runtime_mod {
                tracing::info!(target:"shroudforge::runtime", mod_id=%id, "Mod changes require the next game start");
            }
            if metadata_unchanged && applied["enabled"] == desired["enabled"] && applied["settingValues"] == desired["settingValues"] {
                let mut active_manifest = manifest.clone();
                active_manifest.enabled = applied["enabled"].as_bool().unwrap_or(false);
                active_manifest.setting_values = applied["settingValues"].clone();
                self.loaded.insert(id.clone(),mod_loader::config::revision(&serde_json::to_vec(&active_manifest).expect("manifest is serializable")).into());
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
                let started=std::time::Instant::now();
                let result = self
                    .runner
                    .lua
                    .registry_value::<Function>(key)
                    .and_then(|callback| callback.call::<()>(()));
                self.diagnostics.measure("mods",&item.id,"on_unload",started.elapsed(),result.is_err());
                if let Err(error) = result {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %item.id,
                        "on_unload failed: {error}");
                }
            }
        }
        self.publish_status(false);
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
