use std::{
    cell::{Cell, RefCell, RefMut},
    collections::{
        HashMap, HashSet,
        hash_map::{self, Entry},
    },
    io::Read,
    path::PathBuf as NativePathBuf,
    rc::Rc,
    sync::mpsc::{self, Receiver, TryRecvError},
};

use bitflags::bitflags;
use kfc::{
    container::{KFCCursor, KFCFile, KFCReader, KFCWriter},
    guid::{ContentHash, ResourceId},
    reflection::{TypeHandle, TypeIndex, TypeRegistry},
    resource::value::Value,
};
use mod_loader::{Capability, Mod, ModEnvironment};
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
    log::{error, warn},
    lua::{LuaError, LuaValue},
};

pub struct AppState {
    pub(crate) runtime_configured: Cell<bool>,
    runtime_active_mods: RefCell<HashSet<String>>,
    runtime_effects: RefCell<HashMap<String, serde_json::Value>>,
    native_dlls: RefCell<Vec<NativeDll>>,
    pending_native_dlls: RefCell<Vec<PendingNativeDll>>,
    env: ModEnvironment,
    api: ShroudForgeApi,
    config: AppConfig,

    type_registry: Rc<TypeRegistry>,
    attribute_catalog: OnceCell<Result<super::runtime_attributes::Catalog, String>>,

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
        const PATCH = 1 << 0;
        const EXPORT = 1 << 1;
        const RUNTIME_DLL = 1 << 2;

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
        let game_root = env.game_dir().as_std_path();
        let default_export_dir = PathBuf::from_path_buf(mod_loader::paths::export_dir(game_root))
            .expect("a UTF-8 game path joined with fixed loader paths remains UTF-8");
        let configured_export_dir = options.export_dir.and_then(|directory| {
            use std::path::Component;
            if directory.is_absolute()
                || directory.components().any(|component| {
                    !matches!(component, Component::Normal(_) | Component::CurDir)
                })
            {
                warn!(
                    path = %directory.display(),
                    "Export directory must be a relative path inside the game directory"
                );
                return None;
            }
            let directory = game_root.join(directory);
            if let Err(error) = std::fs::create_dir_all(&directory) {
                warn!(error = %error, path = %directory.display(), "Failed to create export directory");
                return None;
            }
            let canonical_root = match std::fs::canonicalize(game_root) {
                Ok(path) => path,
                Err(error) => {
                    warn!(error = %error, "Failed to resolve game directory for export path validation");
                    return None;
                }
            };
            let canonical_directory = match std::fs::canonicalize(&directory) {
                Ok(path) => path,
                Err(error) => {
                    warn!(error = %error, "Failed to resolve export directory");
                    return None;
                }
            };
            if !canonical_directory.starts_with(&canonical_root) {
                warn!(path = %directory.display(), "Export directory resolves outside the game directory");
                return None;
            }
            PathBuf::from_path_buf(canonical_directory).ok()
        });
        let export_dir = configured_export_dir.unwrap_or(default_export_dir);

        let skip_cache = options.skip_cache;
        let phase = options.phase;
        let is_server = options
            .is_server
            .unwrap_or_else(|| file_name.to_lowercase().contains("server"));

        let mut feature_flags = AppFeatures::empty();

        if options.patch {
            feature_flags |= AppFeatures::PATCH;
        }

        if options.export {
            feature_flags |= AppFeatures::EXPORT;
        }

        #[cfg(windows)]
        if phase == RuntimePhase::Ingame {
            feature_flags |= AppFeatures::RUNTIME_DLL;
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
                        error!(%error, "Invalid installed API contract");
                    })?;
                let schema = shroudforge_parser::schema_from_registry(&type_registry, &ref_file);
                ShroudForgeApi::new(
                    shroudforge_compatibility::Compatibility::new(operations).resolve(schema),
                )
            }
        };
        let writer = if options.patch {
            let stage = shroudforge_parser::transaction::begin(game_dir.as_std_path(), file_name)
                .map_err(|error| {
                error!(error = %error, "Failed to begin typed asset transaction");
            })?;
            let staged_writer = (|| {
                for suffix in ["kfc", "kfc_resources"] {
                    let name = format!("{file_name}.{suffix}");
                    std::fs::copy(game_dir.join(&name), stage.join(&name)).map_err(|error| {
                        error!(error = %error, file = %name, "Failed to stage game asset container");
                    })?;
                }
                let stage = PathBuf::from_path_buf(stage.clone()).map_err(|_| {
                    error!("Asset staging path is not valid UTF-8");
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
            runtime_active_mods: RefCell::new(HashSet::new()),
            runtime_effects: RefCell::new(HashMap::new()),
            native_dlls: RefCell::new(Vec::new()),
            pending_native_dlls: RefCell::new(Vec::new()),
            env,
            api,
            config,

            type_registry,
            attribute_catalog: OnceCell::new(),

            ref_file,
            reader: RefCell::new(reader),
            writer: RefCell::new(writer),
            asset_transaction: Cell::new(options.patch),
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

    pub(crate) fn set_runtime_mod_active(&self, id: &str, active: bool) {
        let native_dll_state = self.native_dll_state(id);
        let mut active_mods = self.runtime_active_mods.borrow_mut();
        if active {
            active_mods.insert(id.to_owned());
        } else {
            active_mods.remove(id);
        }
        drop(active_mods);
        if let Some(state) = native_dll_state {
            let detail = match state {
                "loading" => "native DLL is still loading",
                _ => "native DLL remains loaded for this game session",
            };
            self.report_runtime_effect(id, state, detail);
        } else {
            let mut effects = self.runtime_effects.borrow_mut();
            if effects
                .get(id)
                .is_some_and(|effect| effect["state"] != "restart-required")
            {
                effects.remove(id);
            }
        }
    }

    fn native_dll_state(&self, id: &str) -> Option<&'static str> {
        if self.native_dlls.borrow().iter().any(|dll| dll.mod_id == id) {
            Some("loaded")
        } else if self
            .pending_native_dlls
            .borrow()
            .iter()
            .any(|dll| dll.mod_id == id)
        {
            Some("loading")
        } else {
            None
        }
    }

    pub(crate) fn runtime_mod_is_active(&self, id: &str) -> bool {
        self.runtime_active_mods.borrow().contains(id)
    }

    pub(crate) fn report_runtime_effect(&self, id: &str, state: &str, detail: &str) {
        let value = serde_json::json!({
            "state": state,
            "processTarget": if self.is_server() { "server" } else { "client" },
            "scope": "this-process",
            "detail": detail.chars().take(240).collect::<String>(),
            "updatedAt": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });
        let mut effects = self.runtime_effects.borrow_mut();
        if effects.get(id).is_some_and(|previous| {
            previous["state"] == value["state"] && previous["detail"] == value["detail"]
        }) {
            return;
        }
        effects.insert(id.to_owned(), value);
    }

    pub(crate) fn runtime_effects(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.runtime_effects
                .borrow()
                .iter()
                .map(|(id, value)| (id.clone(), value.clone()))
                .collect(),
        )
    }

    #[inline]
    pub fn export_dir(&self) -> &PathBuf {
        &self.config.export_dir
    }

    #[inline]
    pub fn has_feature(&self, feature: AppFeatures) -> bool {
        self.config.feature_flags.contains(feature)
    }

    pub(crate) fn load_mod_native_dll(
        &self,
        target_mod: &Mod,
        relative_path: &str,
    ) -> Result<(), String> {
        if !target_mod.info().enabled {
            self.report_runtime_effect(
                &target_mod.info().id,
                "restart-required",
                "Native DLL was skipped because the mod is disabled. Enable it and restart the game to load the DLL.",
            );
            tracing::info!(target: "shroudforge::runtime", mod_id = %target_mod.info().id,
                dll = %relative_path, "Native DLL skipped because mod is disabled; restart required after enabling");
            return Ok(());
        }
        self.queue_mod_native_dll(target_mod, relative_path)
    }

    fn queue_mod_native_dll(&self, target_mod: &Mod, relative_path: &str) -> Result<(), String> {
        if !self.has_feature(AppFeatures::RUNTIME_DLL) {
            return Err("native DLL loading is unavailable in this execution phase".into());
        }
        let package_path = camino::Utf8Path::new(relative_path);
        if package_path.as_str().is_empty()
            || package_path.is_absolute()
            || relative_path.contains(':')
            || package_path.components().any(|part| part.as_str() == "..")
        {
            return Err("DLL path must stay inside the mod package".into());
        }
        let mut filesystem = target_mod.fs();
        let (load_path, temporary_directory) = if let Some(path) = filesystem
            .absolute_path(package_path)
            .map_err(|error| error.to_string())?
        {
            if !path.is_file() {
                return Err(format!("registered DLL does not exist: {path}"));
            }
            (path.into_std_path_buf(), None)
        } else {
            let directory = std::env::temp_dir()
                .join("shroudforge")
                .join("native-mods")
                .join(target_mod.info().id.as_str())
                .join(uuid::Uuid::new_v4().to_string());
            std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let extraction = (|| {
                for file in filesystem.files() {
                    let file = file.map_err(|error| error.to_string())?;
                    if file.is_absolute() || file.components().any(|part| part.as_str() == "..") {
                        continue;
                    }
                    let destination = directory.join(file.as_std_path());
                    if let Some(parent) = destination.parent() {
                        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    let mut reader = filesystem
                        .read_file(&file)
                        .map_err(|error| error.to_string())?;
                    let mut output =
                        std::fs::File::create(&destination).map_err(|error| error.to_string())?;
                    std::io::copy(&mut reader, &mut output).map_err(|error| error.to_string())?;
                }
                let destination = directory.join(package_path.as_std_path());
                if !destination.is_file() {
                    return Err("registered DLL was not found in the mod archive".into());
                }
                Ok(destination)
            })();
            match extraction {
                Ok(path) => (path, Some(directory.clone())),
                Err(error) => {
                    let _ = std::fs::remove_dir_all(directory);
                    return Err(error);
                }
            }
        };
        let canonical = match load_path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                if let Some(directory) = temporary_directory {
                    let _ = std::fs::remove_dir_all(directory);
                }
                return Err(format!("could not resolve DLL path: {error}"));
            }
        };
        let mod_id = target_mod.info().id.to_string();
        let already_loaded = {
            self.native_dlls
                .borrow()
                .iter()
                .find(|item| item.path == canonical)
                .is_some()
        };
        if already_loaded {
            if let Some(directory) = temporary_directory {
                let _ = std::fs::remove_dir_all(directory);
            }
            return Ok(());
        }
        if let Some(pending) = self
            .pending_native_dlls
            .borrow_mut()
            .iter_mut()
            .find(|item| item.path == canonical)
        {
            if let Some(directory) = temporary_directory {
                let _ = std::fs::remove_dir_all(directory);
            }
            return Ok(());
        }
        let (sender, receiver) = mpsc::channel();
        let worker_mod_id = mod_id.clone();
        let worker_path = canonical.clone();
        let worker_tempdir = temporary_directory.clone();
        tracing::info!(target: "shroudforge::runtime", mod_id, dll = %canonical.display(),
            "Queuing native DLL load on isolated worker");
        std::thread::Builder::new()
            .name(format!(
                "sf-native-{}",
                mod_id.chars().take(24).collect::<String>()
            ))
            .spawn(move || {
                tracing::info!(target: "shroudforge::runtime", mod_id = %worker_mod_id,
                    dll = %worker_path.display(), "Isolated native DLL load started");
                let result = load_native_library(&worker_mod_id, &worker_path);
                match &result {
                    Ok(_) => {
                        tracing::info!(target: "shroudforge::runtime", mod_id = %worker_mod_id,
                        dll = %worker_path.display(), "Isolated native DLL load completed")
                    }
                    Err(error) => {
                        tracing::error!(target: "shroudforge::runtime", mod_id = %worker_mod_id,
                        dll = %worker_path.display(), "Isolated native DLL load failed: {error}")
                    }
                }
                if let Err(send_error) = sender.send(result) {
                    if let Ok(handle) = send_error.0 {
                        unload_native_library(handle);
                    }
                    if let Some(directory) = worker_tempdir {
                        let _ = std::fs::remove_dir_all(directory);
                    }
                }
            })
            .map_err(|error| {
                if let Some(directory) = &temporary_directory {
                    let _ = std::fs::remove_dir_all(directory);
                }
                format!("could not start isolated DLL loader thread: {error}")
            })?;
        self.pending_native_dlls
            .borrow_mut()
            .push(PendingNativeDll {
                mod_id,
                path: canonical,
                temporary_directory,
                queued_at: std::time::Instant::now(),
                warned_stall: false,
                receiver,
            });
        Ok(())
    }

    pub(crate) fn poll_native_dll_loads(&self) {
        let mut completed = Vec::new();
        {
            let mut pending = self.pending_native_dlls.borrow_mut();
            let mut index = 0;
            while index < pending.len() {
                match pending[index].receiver.try_recv() {
                    Ok(result) => completed.push((pending.remove(index), Some(result))),
                    Err(TryRecvError::Disconnected) => {
                        completed.push((pending.remove(index), None))
                    }
                    Err(TryRecvError::Empty) => {
                        if !pending[index].warned_stall
                            && pending[index].queued_at.elapsed()
                                >= std::time::Duration::from_secs(5)
                        {
                            tracing::error!(target: "shroudforge::runtime", mod_id = %pending[index].mod_id,
                                dll = %pending[index].path.display(), elapsed_ms = pending[index].queued_at.elapsed().as_millis(),
                                "Native DLL load is still running on isolated worker; runtime updates continue");
                            pending[index].warned_stall = true;
                        }
                        index += 1;
                    }
                }
            }
        }
        for (item, result) in completed {
            match result
                .unwrap_or_else(|| Err("native DLL loader worker exited without a result".into()))
            {
                Ok(handle) => {
                    self.native_dlls.borrow_mut().push(NativeDll {
                        mod_id: item.mod_id.clone(),
                        path: item.path.clone(),
                        handle,
                        temporary_directory: item.temporary_directory.clone(),
                    });
                    tracing::info!(target: "shroudforge::runtime", mod_id = %item.mod_id,
                        dll = %item.path.display(), "Native DLL load completed");
                    self.report_runtime_effect(&item.mod_id, "loaded", "native DLL loaded");
                }
                Err(error) => {
                    if let Some(directory) = item.temporary_directory {
                        let _ = std::fs::remove_dir_all(directory);
                    }
                    tracing::error!(target: "shroudforge::runtime", mod_id = %item.mod_id,
                        dll = %item.path.display(), "Native DLL load failed: {error}");
                    self.report_runtime_effect(&item.mod_id, "error", &error);
                }
            }
        }
    }

    pub(crate) fn load_native_dll_declaration(&self, target_mod: &Mod) -> Result<bool, String> {
        if !self.has_feature(AppFeatures::RUNTIME_DLL)
            || !target_mod
                .info()
                .capabilities
                .contains(&Capability::RuntimeRegisterDll)
        {
            return Ok(false);
        }
        let config_path = camino::Utf8Path::new("native-plugin.ini");
        let config = {
            let mut filesystem = target_mod.fs();
            if !filesystem.is_file(config_path) {
                return Ok(false);
            }
            let mut config = String::new();
            filesystem
                .read_file(config_path)
                .map_err(|error| format!("could not read native-plugin.ini: {error}"))?
                .read_to_string(&mut config)
                .map_err(|error| format!("could not decode native-plugin.ini: {error}"))?;
            config
        };
        let mut in_plugin_section = false;
        let mut enabled = false;
        let mut dll = None;
        for raw_line in config.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                in_plugin_section = line[1..line.len() - 1]
                    .trim()
                    .eq_ignore_ascii_case("Plugin");
                continue;
            }
            if !in_plugin_section {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            let value = if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                &value[1..value.len() - 1]
            } else {
                value
            };
            match key.trim().to_ascii_lowercase().as_str() {
                "enabled" => {
                    enabled = matches!(
                        value.to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                }
                "dll" => dll = Some(value.to_owned()),
                _ => {}
            }
        }
        if !enabled {
            return Ok(false);
        }
        let dll = dll.ok_or_else(|| {
            "native-plugin.ini enables DLL loading but does not name a DLL".to_owned()
        })?;
        tracing::info!(target: "shroudforge::runtime", mod_id = %target_mod.info().id,
            dll = %dll, "Native DLL declaration parsed; queuing isolated load");
        self.queue_mod_native_dll(target_mod, &dll)?;
        Ok(true)
    }

    fn unload_native_dll(&self, handle: isize) {
        let mut loaded = self.native_dlls.borrow_mut();
        let Some(index) = loaded.iter().position(|item| item.handle == handle) else {
            return;
        };
        let dll = loaded.remove(index);
        drop(loaded);
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::FreeLibrary(
                dll.handle as windows_sys::Win32::Foundation::HMODULE,
            );
        }
        if let Some(directory) = dll.temporary_directory {
            if let Err(error) = std::fs::remove_dir_all(directory) {
                tracing::warn!(target: "shroudforge::runtime", %error, "Could not remove extracted native DLL files");
            }
        }
    }

    #[inline]
    pub fn kfc_file(&self) -> &KFCFile {
        &self.ref_file
    }

    #[inline]
    pub fn type_registry(&self) -> &Rc<TypeRegistry> {
        &self.type_registry
    }

    pub(crate) fn attribute_catalog(&self) -> Result<&super::runtime_attributes::Catalog, String> {
        self.attribute_catalog
            .get_or_init(|| {
                super::runtime_attributes::Catalog::load(
                    &self.type_registry,
                    &self.ref_file,
                    &mut self.reader.borrow_mut(),
                )
            })
            .as_ref()
            .map_err(Clone::clone)
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
            Ok(self.reader.borrow_mut().read_content(guid)?)
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
        for dll in self.native_dlls.get_mut().drain(..).rev() {
            #[cfg(windows)]
            unsafe {
                windows_sys::Win32::Foundation::FreeLibrary(
                    dll.handle as windows_sys::Win32::Foundation::HMODULE,
                );
            }
            if let Some(directory) = dll.temporary_directory {
                if let Err(error) = std::fs::remove_dir_all(directory) {
                    tracing::warn!(target: "shroudforge::runtime", %error, "Could not remove extracted native DLL files");
                }
            }
        }
    }
}

struct NativeDll {
    mod_id: String,
    path: NativePathBuf,
    #[allow(dead_code)]
    handle: isize,
    temporary_directory: Option<NativePathBuf>,
}

struct PendingNativeDll {
    mod_id: String,
    path: NativePathBuf,
    temporary_directory: Option<NativePathBuf>,
    queued_at: std::time::Instant,
    warned_stall: bool,
    receiver: Receiver<Result<isize, String>>,
}

#[cfg(windows)]
fn load_native_library(mod_id: &str, path: &std::path::Path) -> Result<isize, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::{
        LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
    };

    tracing::debug!(target: "shroudforge::runtime", mod_id, dll = %path.display(),
        "Calling LoadLibraryExW on isolated worker");
    let wide_path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        LoadLibraryExW(
            wide_path.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        )
    };
    if handle.is_null() {
        Err(format!(
            "Windows could not load native DLL for mod '{mod_id}': {}",
            std::io::Error::last_os_error()
        ))
    } else {
        tracing::debug!(target: "shroudforge::runtime", mod_id, dll = %path.display(),
            "LoadLibraryExW returned on isolated worker");
        Ok(handle as isize)
    }
}

#[cfg(windows)]
fn unload_native_library(handle: isize) {
    unsafe {
        windows_sys::Win32::Foundation::FreeLibrary(
            handle as windows_sys::Win32::Foundation::HMODULE,
        );
    }
}

#[cfg(not(windows))]
fn load_native_library(mod_id: &str, _path: &std::path::Path) -> Result<isize, String> {
    Err(format!(
        "native DLL loading for mod '{mod_id}' is supported only on Windows"
    ))
}

#[cfg(not(windows))]
fn unload_native_library(_handle: isize) {}

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
