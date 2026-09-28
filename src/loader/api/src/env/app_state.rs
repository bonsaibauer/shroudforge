use std::{
    cell::{Cell, RefCell, RefMut},
    collections::{
        HashMap, HashSet,
        hash_map::{self, Entry},
    },
    io::Read,
    path::PathBuf as NativePathBuf,
    rc::Rc,
};

use bitflags::bitflags;
use kfc::{
    container::{KFCCursor, KFCFile, KFCReader, KFCWriter},
    guid::{ContentHash, ResourceId},
    reflection::{TypeHandle, TypeIndex, TypeRegistry},
    resource::value::Value,
};
use mod_loader::{Mod, ModEnvironment};
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
    runtime_active_mods: RefCell<HashSet<String>>,
    runtime_effects: RefCell<HashMap<String, serde_json::Value>>,
    native_dlls: RefCell<Vec<NativeDll>>,
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
            .unwrap_or_else(|| {
                PathBuf::from_path_buf(mod_loader::paths::export_dir(env.game_dir().as_std_path()))
                    .expect("a UTF-8 game path joined with fixed loader paths remains UTF-8")
            });

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
                        warn!(%error, "Invalid installed API contract");
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
            runtime_active_mods: RefCell::new(HashSet::new()),
            runtime_effects: RefCell::new(HashMap::new()),
            native_dlls: RefCell::new(Vec::new()),
            env,
            api,
            config,

            type_registry,

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
        let mut active_mods = self.runtime_active_mods.borrow_mut();
        if active {
            active_mods.insert(id.to_owned());
            self.runtime_effects.borrow_mut().remove(id);
        } else {
            active_mods.remove(id);
            self.runtime_effects.borrow_mut().remove(id);
        }
    }

    pub(crate) fn runtime_mod_is_active(&self, id: &str) -> bool {
        self.runtime_active_mods.borrow().contains(id)
    }

    pub(crate) fn report_runtime_effect(&self, id: &str, state: &str, detail: &str) {
        let value = serde_json::json!({
            "state": state,
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

    pub(crate) fn load_native_dll(
        &self,
        mod_id: &str,
        path: &std::path::Path,
        temporary_directory: Option<NativePathBuf>,
    ) -> Result<isize, String> {
        if !self.has_feature(AppFeatures::RUNTIME_DLL) {
            return Err("native DLL loading is unavailable in this execution phase".into());
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("could not resolve DLL path: {error}"))?;
        if !canonical.is_file() {
            return Err(format!(
                "native DLL does not exist: {}",
                canonical.display()
            ));
        }
        let mut loaded = self.native_dlls.borrow_mut();
        if let Some(item) = loaded.iter().find(|item| item.path == canonical) {
            return Ok(item.handle);
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::System::LibraryLoader::{
                LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
            };
            let wide_path = canonical
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
                return Err(format!(
                    "Windows could not load native DLL for mod '{mod_id}': {}",
                    std::io::Error::last_os_error()
                ));
            }
            loaded.push(NativeDll {
                mod_id: mod_id.to_owned(),
                path: canonical,
                handle: handle as isize,
                temporary_directory,
                plugin_id: None,
                stop_function: None,
                mod_root_utf16: None,
            });
            tracing::info!(target: "shroudforge::runtime", mod_id, dll = %path.display(), "Loaded EML native DLL");
            Ok(handle as isize)
        }
        #[cfg(not(windows))]
        {
            let _ = (mod_id, canonical, temporary_directory);
            Err("EML native DLL loading is supported only on Windows".into())
        }
    }

    pub(crate) fn load_mod_native_dll(
        &self,
        target_mod: &Mod,
        relative_path: &str,
    ) -> Result<(), String> {
        self.load_mod_native_dll_impl(target_mod, relative_path, false)
            .map(|_| ())
    }

    fn load_mod_native_dll_impl(
        &self,
        target_mod: &Mod,
        relative_path: &str,
        require_plugin_host: bool,
    ) -> Result<bool, String> {
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
        let package_directory = filesystem.root().as_std_path().to_path_buf();
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
        let mod_directory = temporary_directory.clone().unwrap_or(package_directory);
        match self.load_native_dll(
            &target_mod.info().id,
            &load_path,
            temporary_directory.clone(),
        ) {
            Ok(handle) => {
                match self.start_native_plugin(
                    target_mod,
                    handle,
                    &mod_directory,
                    require_plugin_host,
                ) {
                    Ok(started) => Ok(started),
                    Err(error) => {
                        self.unload_native_dll(handle);
                        Err(error)
                    }
                }
            }
            Err(error) => {
                if let Some(directory) = temporary_directory {
                    let _ = std::fs::remove_dir_all(directory);
                }
                Err(error)
            }
        }
    }

    pub(crate) fn load_native_plugin(&self, target_mod: &Mod) -> Result<bool, String> {
        if !self.has_feature(AppFeatures::RUNTIME_DLL) {
            return Ok(false);
        }
        let mut filesystem = target_mod.fs();
        let config_path = camino::Utf8Path::new("native-plugin.ini");
        if !filesystem.is_file(config_path) {
            return Ok(false);
        }
        let mut config = String::new();
        filesystem
            .read_file(config_path)
            .map_err(|error| format!("could not read native-plugin.ini: {error}"))?
            .read_to_string(&mut config)
            .map_err(|error| format!("could not decode native-plugin.ini: {error}"))?;

        let mut in_plugin_section = false;
        let mut enabled = false;
        let mut dll = None;
        for raw_line in config.lines() {
            let line = raw_line
                .split(|character| character == ';' || character == '#')
                .next()
                .unwrap_or_default()
                .trim();
            if line.is_empty() {
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
            let value = value.trim().trim_matches('"').trim_matches('\'');
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
            "native-plugin.ini enables the plugin but does not name a DLL".to_owned()
        })?;
        if !self.load_mod_native_dll_impl(target_mod, &dll, true)? {
            return Err("native-plugin.ini sidecar did not start a supported native plugin".into());
        }
        Ok(true)
    }

    #[cfg(windows)]
    fn start_native_plugin(
        &self,
        target_mod: &Mod,
        handle: isize,
        mod_directory: &std::path::Path,
        required: bool,
    ) -> Result<bool, String> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{
            Foundation::HMODULE,
            System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
        };

        type Query = unsafe extern "system" fn(*mut NativePluginInfoV1) -> i32;
        type Start = unsafe extern "system" fn(*const NativePluginHostV1) -> i32;
        type Stop = unsafe extern "system" fn();

        if self
            .native_dlls
            .borrow()
            .iter()
            .any(|item| item.handle == handle && item.stop_function.is_some())
        {
            return Ok(true);
        }

        let module = handle as HMODULE;
        let query = unsafe { GetProcAddress(module, NATIVE_PLUGIN_QUERY_EXPORT.as_ptr()) };
        let start = unsafe { GetProcAddress(module, NATIVE_PLUGIN_START_EXPORT.as_ptr()) };
        let stop = unsafe { GetProcAddress(module, NATIVE_PLUGIN_STOP_EXPORT.as_ptr()) };
        if query.is_none() && start.is_none() && stop.is_none() {
            return if required {
                Err("native-plugin.ini sidecar does not export the XHL host ABI v1".into())
            } else {
                Ok(false)
            };
        }
        let (Some(query), Some(start), Some(stop)) = (query, start, stop) else {
            return Err("native plugin exports an incomplete XHL host ABI v1".into());
        };
        let query: Query = unsafe { std::mem::transmute(query) };
        let start: Start = unsafe { std::mem::transmute(start) };
        let stop: Stop = unsafe { std::mem::transmute(stop) };

        let mut descriptor = NativePluginInfoV1 {
            size: std::mem::size_of::<NativePluginInfoV1>() as u32,
            ..NativePluginInfoV1::default()
        };
        if unsafe { query(&mut descriptor) } == 0 {
            return Err("native plugin rejected the XHL host ABI v1 descriptor".into());
        }
        if descriptor.size as usize != std::mem::size_of::<NativePluginInfoV1>()
            || descriptor.abi_version != 0x0001_0000
            || descriptor.reserved != 0
        {
            return Err("native plugin reported an unsupported XHL ABI version".into());
        }
        let plugin_id = native_plugin_text_field(&descriptor.id, "ID")?;
        let _plugin_name = native_plugin_text_field(&descriptor.name, "name")?;
        let _plugin_version = native_plugin_text_field(&descriptor.version, "version")?;
        if plugin_id.is_empty()
            || !plugin_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("native plugin descriptor ID contains unsupported characters".into());
        }
        if !plugin_id.eq_ignore_ascii_case(&target_mod.info().id) {
            return Err(format!(
                "native plugin ID '{plugin_id}' does not match mod '{}'",
                target_mod.info().id
            ));
        }
        let loaded = self.native_dlls.borrow();
        if loaded.iter().any(|item| {
            item.handle != handle
                && item
                    .plugin_id
                    .as_deref()
                    .is_some_and(|loaded_id| loaded_id.eq_ignore_ascii_case(plugin_id))
        }) {
            return Err(format!("native plugin ID '{plugin_id}' is already loaded"));
        }
        if loaded
            .iter()
            .filter(|item| item.plugin_id.is_some())
            .count()
            >= 16
        {
            return Err("native plugin host limit of 16 loaded plugins was reached".into());
        }
        drop(loaded);

        let mut wide_directory = mod_directory.as_os_str().encode_wide().collect::<Vec<_>>();
        wide_directory.push(0);
        let wide_directory = wide_directory.into_boxed_slice();
        let game_module = unsafe { GetModuleHandleW(std::ptr::null()) };
        if game_module.is_null() {
            return Err(format!(
                "could not resolve the game module: {}",
                std::io::Error::last_os_error()
            ));
        }
        let context = NativePluginHostV1 {
            size: std::mem::size_of::<NativePluginHostV1>() as u32,
            abi_version: 0x0001_0000,
            game_module,
            mod_directory: wide_directory.as_ptr(),
            process_id: unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() },
            reserved: 0,
            log: Some(native_plugin_log),
        };
        if unsafe { start(&context) } == 0 {
            return Err("native plugin start callback failed for XHL host ABI v1".into());
        }

        let mut loaded = self.native_dlls.borrow_mut();
        let library = loaded
            .iter_mut()
            .find(|item| item.handle == handle)
            .ok_or_else(|| "native plugin handle was lost during startup".to_owned())?;
        library.stop_function = Some(stop);
        library.plugin_id = Some(plugin_id.to_owned());
        library.mod_root_utf16 = Some(wide_directory);
        tracing::info!(target: "shroudforge::runtime", mod_id = %target_mod.info().id,
            "Started native plugin using XHL host ABI v1");
        Ok(true)
    }

    #[cfg(not(windows))]
    fn start_native_plugin(
        &self,
        _target_mod: &Mod,
        _handle: isize,
        _mod_directory: &std::path::Path,
        _required: bool,
    ) -> Result<bool, String> {
        Err("native plugin ABI v1 is supported only on Windows".into())
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
            if let Some(stop) = dll.stop_function {
                stop();
            }
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
                if let Some(stop) = dll.stop_function {
                    stop();
                }
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
    #[allow(dead_code)]
    mod_id: String,
    path: NativePathBuf,
    #[allow(dead_code)]
    handle: isize,
    temporary_directory: Option<NativePathBuf>,
    plugin_id: Option<String>,
    #[allow(dead_code)]
    stop_function: Option<unsafe extern "system" fn()>,
    #[allow(dead_code)]
    mod_root_utf16: Option<Box<[u16]>>,
}

// Internal names describe the host role. This layout currently adapts the XHL
// native-plugin.ini ABI; the exported symbol names below remain XHL-defined.
const NATIVE_PLUGIN_QUERY_EXPORT: &[u8] = b"XhlNativePluginQuery\0";
const NATIVE_PLUGIN_START_EXPORT: &[u8] = b"XhlNativePluginStart\0";
const NATIVE_PLUGIN_STOP_EXPORT: &[u8] = b"XhlNativePluginRequestStop\0";

#[repr(C)]
struct NativePluginInfoV1 {
    size: u32,
    abi_version: u32,
    reserved: u32,
    id: [u8; 64],
    name: [u8; 128],
    version: [u8; 32],
}

impl Default for NativePluginInfoV1 {
    fn default() -> Self {
        Self {
            size: 0,
            abi_version: 0,
            reserved: 0,
            id: [0; 64],
            name: [0; 128],
            version: [0; 32],
        }
    }
}

fn native_plugin_text_field<'a>(field: &'a [u8], name: &str) -> Result<&'a str, String> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("native plugin descriptor {name} is not NUL-terminated"))?;
    if field[..end]
        .iter()
        .any(|byte| *byte < 0x20 || *byte == 0x7f)
    {
        return Err(format!(
            "native plugin descriptor {name} contains non-printable bytes"
        ));
    }
    std::str::from_utf8(&field[..end])
        .map_err(|_| format!("native plugin descriptor {name} is not UTF-8"))
}

#[cfg(windows)]
#[repr(C)]
struct NativePluginHostV1 {
    size: u32,
    abi_version: u32,
    game_module: windows_sys::Win32::Foundation::HMODULE,
    mod_directory: *const u16,
    process_id: u32,
    reserved: u32,
    log: Option<unsafe extern "system" fn(*const std::ffi::c_char)>,
}

#[cfg(windows)]
unsafe extern "system" fn native_plugin_log(message: *const std::ffi::c_char) {
    if message.is_null() {
        return;
    }
    let message = unsafe { std::ffi::CStr::from_ptr(message) }.to_string_lossy();
    tracing::info!(target: "shroudforge::native-plugin", "{message}");
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
