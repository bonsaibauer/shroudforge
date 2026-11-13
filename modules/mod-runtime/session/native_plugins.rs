use super::*;
use crate::session::native_plugin_abi::{
    NativePluginEntry, NativePluginHostMetadata, NativePluginInstance, NativePluginMode,
};
use mod_loader::Capability;
use mod_loader::dll_manager::{DllLoadMode, PluginLoadDisposition};
use std::path::PathBuf as NativePathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::{cell::RefCell, io::Read};

static NEXT_NATIVE_PACKAGE_GENERATION: AtomicU64 = AtomicU64::new(1);

fn spawn_retained_native_worker(
    name: String,
    retain_module: impl FnOnce() -> Result<(), String>,
    work: impl FnOnce() + Send + 'static,
) -> Result<std::thread::JoinHandle<()>, String> {
    // Pending workers and quarantined plugin callbacks can outlive AppState.
    // Pin their owning runtime before any code or host pointer escapes.
    retain_module()?;
    std::thread::Builder::new()
        .name(name)
        .spawn(work)
        .map_err(|error| format!("could not start isolated DLL loader thread: {error}"))
}

fn next_native_package_generation() -> u64 {
    NEXT_NATIVE_PACKAGE_GENERATION
        .fetch_add(1, Ordering::Relaxed)
        .max(1)
}

struct StagedNativePlugin {
    mod_id: String,
    entry: mod_loader::dll_manager::DllLoadEntry,
    path: NativePathBuf,
    package_directory: String,
    package_root: NativePathBuf,
    declaration: mod_loader::dll_manager::PluginIniDeclaration,
    package: Mod,
    settings_json: Vec<u8>,
    package_generation: u64,
    bridge_initialized: bool,
    library: NativeLibrary,
    instance: NativePluginInstance,
}

struct StagedNativePluginBatch<Plugin = StagedNativePlugin> {
    root: NativePathBuf,
    is_server: bool,
    services: Arc<dyn crate::session::native_plugin_abi::NativePluginServiceLifetime>,
    plugins: Vec<Plugin>,
}

struct LoadedNativeLibrary {
    library: NativeLibrary,
    bridge_initialized: bool,
}

enum NativeLibraryLoadError {
    Load(String),
    Bridge {
        library: NativeLibrary,
        detail: String,
    },
}

impl NativeLibraryLoadError {
    fn detail(&self) -> &str {
        match self {
            Self::Load(detail) | Self::Bridge { detail, .. } => detail,
        }
    }
}

enum SelectedNativePluginState<Plugin = StagedNativePlugin> {
    Empty,
    Loading,
    Loaded(StagedNativePluginBatch<Plugin>),
    Staging,
    Staged(StagedNativePluginBatch<Plugin>),
    Preparing,
    Prepared(StagedNativePluginBatch<Plugin>),
    Adopting,
    Adopted {
        root: NativePathBuf,
        is_server: bool,
    },
    Failed,
}

struct SelectedNativePluginsThreadOwner<Plugin = StagedNativePlugin> {
    state: RefCell<SelectedNativePluginState<Plugin>>,
}

impl<Plugin> SelectedNativePluginsThreadOwner<Plugin> {
    const fn new() -> Self {
        Self {
            state: RefCell::new(SelectedNativePluginState::Empty),
        }
    }

    fn try_borrow(
        &self,
    ) -> Result<std::cell::Ref<'_, SelectedNativePluginState<Plugin>>, std::cell::BorrowError> {
        self.state.try_borrow()
    }

    fn try_borrow_mut(
        &self,
    ) -> Result<std::cell::RefMut<'_, SelectedNativePluginState<Plugin>>, std::cell::BorrowMutError>
    {
        self.state.try_borrow_mut()
    }
}

impl<Plugin> Drop for SelectedNativePluginsThreadOwner<Plugin> {
    fn drop(&mut self) {
        let state = std::mem::replace(self.state.get_mut(), SelectedNativePluginState::Failed);
        match state {
            SelectedNativePluginState::Loaded(batch)
            | SelectedNativePluginState::Staged(batch)
            | SelectedNativePluginState::Prepared(batch) => {
                // A restored-entry callback can still be executing on another
                // thread after its owner times out. Retain the complete batch
                // without calling plugin code from this TLS destructor.
                std::mem::forget(batch);
            }
            SelectedNativePluginState::Empty
            | SelectedNativePluginState::Loading
            | SelectedNativePluginState::Staging
            | SelectedNativePluginState::Preparing
            | SelectedNativePluginState::Adopting
            | SelectedNativePluginState::Adopted { .. }
            | SelectedNativePluginState::Failed => {}
        }
    }
}

thread_local! {
    // The loaded host/context and eventual AppState lifecycle share the
    // bootstrap owner thread. No borrowed TLS state crosses a plugin callback.
    static SELECTED_NATIVE_PLUGINS: SelectedNativePluginsThreadOwner =
        const { SelectedNativePluginsThreadOwner::new() };
}

const MAX_ENTRY_RESTORED_CALLBACKS: usize = 64;

#[derive(Clone, Copy)]
struct EntryRestoredCall {
    mod_id: [u8; 256],
    mod_id_len: u16,
    callback: crate::session::native_plugin_abi::NativeEntryRestoredContinuation,
}

struct PublishedEntryRestoredBatch {
    root: NativePathBuf,
    is_server: bool,
    calls: Vec<EntryRestoredCall>,
    retained_modules: Vec<isize>,
}

enum PublishedEntryRestoredState {
    Empty,
    Ready(PublishedEntryRestoredBatch),
    Running,
    Succeeded(PublishedEntryRestoredBatch),
    Failed(PublishedEntryRestoredBatch),
}

static PUBLISHED_ENTRY_RESTORED: OnceLock<Mutex<PublishedEntryRestoredState>> = OnceLock::new();

fn published_entry_restored_state() -> &'static Mutex<PublishedEntryRestoredState> {
    PUBLISHED_ENTRY_RESTORED.get_or_init(|| Mutex::new(PublishedEntryRestoredState::Empty))
}

fn same_native_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn canonical_native_path(path: &std::path::Path) -> Result<NativePathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("could not resolve native startup path: {error}"))
}

fn release_staged_plugin(
    plugin: StagedNativePlugin,
    outcome: Option<crate::session::native_plugin_lifecycle::TeardownOutcome>,
) {
    use crate::session::native_plugin_lifecycle::TeardownOutcome;
    let StagedNativePlugin {
        bridge_initialized,
        mut library,
        instance,
        ..
    } = plugin;
    if matches!(outcome, Some(TeardownOutcome::Retain { .. })) {
        std::mem::forget((library, instance));
        return;
    }
    if bridge_initialized {
        // The legacy bridge registry owns function pointers for process life.
        // Its adapter may gate calls after lifecycle revocation, but the host
        // must keep that module's code mapped.
        std::mem::forget(library);
        drop(instance);
        return;
    }
    if let Err(error) = library.try_free() {
        tracing::error!(target: "shroudforge::runtime", %error,
            "Retaining staged native plugin because its module could not be unloaded");
        std::mem::forget((library, instance));
    }
}

fn cleanup_staged_batch(
    batch: StagedNativePluginBatch,
    failed_index: Option<(
        usize,
        crate::session::native_plugin_lifecycle::TeardownOutcome,
    )>,
) {
    for (index, mut plugin) in batch.plugins.into_iter().enumerate().rev() {
        let outcome = if let Some((failed_index, failed_outcome)) = failed_index
            && failed_index == index
        {
            Some(failed_outcome)
        } else {
            Some(plugin.instance.rollback_preparation())
        };
        release_staged_plugin(plugin, outcome);
    }
}

fn selected_batch_matches(
    batch: &StagedNativePluginBatch,
    root: &std::path::Path,
    is_server: bool,
) -> bool {
    canonical_native_path(root)
        .is_ok_and(|root| same_native_path(&batch.root, &root) && batch.is_server == is_server)
}

#[cfg(windows)]
fn retain_entry_restored_module(address: *const std::ffi::c_void) -> Result<isize, String> {
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleHandleExW,
    };
    let mut module = std::ptr::null_mut();
    let retained = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            address.cast(),
            &mut module,
        )
    };
    if retained == 0 {
        Err(format!(
            "could not retain an entry-restored callback module: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(module as isize)
    }
}

#[cfg(not(windows))]
fn retain_entry_restored_module(_address: *const std::ffi::c_void) -> Result<isize, String> {
    Err("V2 entry-restored callbacks are supported only on Windows".into())
}

fn release_entry_restored_module(module: isize) -> Result<(), String> {
    #[cfg(windows)]
    {
        let released = unsafe {
            windows_sys::Win32::Foundation::FreeLibrary(
                module as windows_sys::Win32::Foundation::HMODULE,
            )
        };
        if released == 0 {
            return Err(format!(
                "could not release an entry-restored callback module reference: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = module;
        Err("native module references are supported only on Windows".into())
    }
}

fn publish_entry_restored_continuations(batch: &StagedNativePluginBatch) -> Result<(), String> {
    let mut calls = Vec::new();
    let mut retained_modules = Vec::new();
    for plugin in &batch.plugins {
        let Some(callback) = plugin.instance.entry_restored_continuation() else {
            continue;
        };
        if calls.len() == MAX_ENTRY_RESTORED_CALLBACKS {
            for module in retained_modules.drain(..) {
                let _ = release_entry_restored_module(module);
            }
            return Err(format!(
                "selected V2 plugins exceed the {MAX_ENTRY_RESTORED_CALLBACKS} entry-restored callback limit"
            ));
        }
        let bytes = plugin.mod_id.as_bytes();
        if bytes.len() > 256 {
            for module in retained_modules.drain(..) {
                let _ = release_entry_restored_module(module);
            }
            return Err("entry-restored plugin identity exceeds its retained bound".into());
        }
        let module = match retain_entry_restored_module(callback.address()) {
            Ok(module) => module,
            Err(error) => {
                for module in retained_modules.drain(..) {
                    let _ = release_entry_restored_module(module);
                }
                return Err(error);
            }
        };
        retained_modules.push(module);
        let mut mod_id = [0u8; 256];
        mod_id[..bytes.len()].copy_from_slice(bytes);
        calls.push(EntryRestoredCall {
            mod_id,
            mod_id_len: bytes.len() as u16,
            callback,
        });
    }
    let mut state = published_entry_restored_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if !matches!(*state, PublishedEntryRestoredState::Empty) {
        drop(state);
        for module in retained_modules.drain(..) {
            let _ = release_entry_restored_module(module);
        }
        return Err("entry-restored callback batch was already published".into());
    }
    *state = PublishedEntryRestoredState::Ready(PublishedEntryRestoredBatch {
        root: batch.root.clone(),
        is_server: batch.is_server,
        calls,
        retained_modules,
    });
    Ok(())
}

pub(crate) fn dispatch_published_entry_restored_callbacks() -> bool {
    let batch = {
        let mut state = published_entry_restored_state()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match std::mem::replace(&mut *state, PublishedEntryRestoredState::Running) {
            PublishedEntryRestoredState::Ready(batch) => batch,
            previous => {
                *state = previous;
                return false;
            }
        }
    };
    let mut success = true;
    for call in &batch.calls {
        let status = call.callback.invoke();
        if status != crate::session::native_plugin_abi::NATIVE_STATUS_OK {
            let mod_id = std::str::from_utf8(&call.mod_id[..usize::from(call.mod_id_len)])
                .unwrap_or("unknown");
            tracing::error!(target: "shroudforge::runtime", mod_id, status,
                "V2 entry-restored callback failed; original game entry will remain denied");
            success = false;
            break;
        }
    }
    let mut state = published_entry_restored_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if matches!(*state, PublishedEntryRestoredState::Running) {
        *state = if success {
            PublishedEntryRestoredState::Succeeded(batch)
        } else {
            PublishedEntryRestoredState::Failed(batch)
        };
        success
    } else {
        // A concurrent deadline or duplicate dispatcher changed authority.
        // Keep the module references process-lived on uncertainty.
        std::mem::forget(batch);
        false
    }
}

pub(super) fn finalize_entry_restored_callbacks(
    root: &std::path::Path,
    is_server: bool,
) -> Result<(), String> {
    let root = canonical_native_path(root)?;
    let batch = {
        let mut state = published_entry_restored_state()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let previous = std::mem::replace(&mut *state, PublishedEntryRestoredState::Empty);
        match previous {
            PublishedEntryRestoredState::Succeeded(batch)
                if same_native_path(&batch.root, &root) && batch.is_server == is_server =>
            {
                batch
            }
            previous => {
                *state = previous;
                return Err("V2 entry-restored callback completion was not confirmed".into());
            }
        }
    };
    for module in batch.retained_modules {
        release_entry_restored_module(module)?;
    }
    Ok(())
}

pub(super) fn load_selected_native_plugins(
    root: &std::path::Path,
    is_server: bool,
    environment: &mod_loader::ModEnvironment,
) -> Result<(), String> {
    let reserved: Result<(), String> = SELECTED_NATIVE_PLUGINS.with(|state| {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
        if !matches!(*state, SelectedNativePluginState::Empty) {
            return Err(
                "Selected native plugins were already staged on this owner thread".to_owned(),
            );
        }
        *state = SelectedNativePluginState::Loading;
        Ok(())
    });
    reserved?;

    let mut plugins = Vec::new();
    let stage_result = (|| {
        let root = canonical_native_path(root)?;
        let selected = crate::native_dlls::deferred_native_plugins(&root, is_server)?;
        let selected_v2: Vec<_> = selected
            .into_iter()
            .filter(|plugin| {
                plugin.declaration.state.is_enabled()
                    && plugin
                        .declaration
                        .lifecycle
                        .as_deref()
                        .is_some_and(|lifecycle| lifecycle.eq_ignore_ascii_case("shroudforge-v2"))
            })
            .collect();
        if selected_v2.is_empty() {
            return Err("Selected V2 pre-entry demand has no enabled native plugin".into());
        }
        if !same_native_path(environment.game_dir().as_std_path(), &root) {
            return Err("Staged package plan belongs to a different game root".into());
        }
        let services = super::new_native_plugin_service_lifetime(environment, is_server);
        for plugin in selected_v2 {
            if plugin.entry.mode != DllLoadMode::Library {
                return Err("Selected V2 native plugins require library mode".into());
            }
            let mod_id = plugin
                .entry
                .mod_id
                .as_deref()
                .ok_or("V2 native plugins require a manifest-backed package")?;
            let package = environment
                .mod_registry()
                .get(mod_id)
                .ok_or("Selected V2 native plugin is absent from the runtime package plan")?
                .clone();
            let package_info = package.info();
            if package_info.id != mod_id
                || !package_info.enabled
                || !package_info.target.supports_process(is_server)
                || !package_info
                    .capabilities
                    .contains(&Capability::RuntimeRegisterDll)
            {
                return Err("Selected V2 native plugin package admission changed".into());
            }
            let package_root = canonical_native_path(package.fs().root().as_std_path())?;
            let package_directory = package_root
                .file_name()
                .ok_or("Selected V2 package folder is unavailable")?
                .to_string_lossy()
                .into_owned();
            if !package_root.is_dir()
                || !package_root.starts_with(&root)
                || !package_directory.eq_ignore_ascii_case(&plugin.entry.package_directory)
            {
                return Err("Selected V2 package root or folder identity changed".into());
            }
            let manifest = mod_loader::config::read_manifest_path(&root, &package_root)
                .map_err(|error| format!("Could not revalidate V2 package manifest: {error}"))?;
            if manifest.id != mod_id
                || !manifest.enabled
                || !manifest.target.supports_process(is_server)
                || !manifest
                    .capabilities
                    .contains(&Capability::RuntimeRegisterDll)
            {
                return Err(
                    "Selected V2 package manifest no longer matches startup admission".into(),
                );
            }
            let declaration = read_native_plugin_declaration(&package)?
                .ok_or("Selected V2 package no longer has native-plugin.ini")?;
            if declaration.metadata != plugin.declaration
                || native_plugin_mode(&declaration.metadata)? != NativePluginMode::ShroudForgeV2
                || !declaration.metadata.state.is_enabled()
            {
                return Err("Selected V2 native plugin declaration changed before staging".into());
            }
            let relative_path = declaration
                .metadata
                .dll
                .as_deref()
                .ok_or("Selected V2 native plugin does not name a DLL")?;
            let selected_path = mod_loader::dll_manager::resolve_package_dll_path(
                &root,
                &package_directory,
                &mod_loader::dll_manager::DllProfileFile {
                    path: relative_path.to_owned(),
                    mode: DllLoadMode::Library,
                },
            )?;
            let path = canonical_native_path(&plugin.entry.path)?;
            if !same_native_path(&selected_path, &path) {
                return Err("V2 DLL path differs from the exact selected startup input".into());
            }
            let settings_json = serde_json::to_vec(&package_info.setting_values)
                .map_err(|error| format!("Could not encode V2 plugin settings: {error}"))?;
            if settings_json.len()
                > crate::session::native_plugin_abi::NATIVE_MAX_SETTINGS_JSON_BYTES
            {
                return Err("V2 native plugin settings JSON exceeds 65536 bytes".into());
            }
            let loaded = isolated_load_native_library(
                mod_id.to_owned(),
                path.clone(),
                None,
                package_root.clone(),
            )
            .map_err(|error| match error {
                NativeLibraryLoadError::Load(detail) => detail,
                NativeLibraryLoadError::Bridge { library, detail } => {
                    // Registration may have retained callbacks into this DLL.
                    std::mem::forget(library);
                    format!("{detail}; native bridge module code is retained until process exit")
                }
            })?;
            let LoadedNativeLibrary {
                library,
                bridge_initialized,
            } = loaded;
            let entry = match library.entrypoint(NativePluginMode::ShroudForgeV2) {
                Ok(Some(entry)) => entry,
                Ok(None) => {
                    retain_or_unload_new_library(library, bridge_initialized);
                    return Err("V2 native plugin is missing ShroudForgePluginEntry".into());
                }
                Err(error) => {
                    retain_or_unload_new_library(library, bridge_initialized);
                    return Err(error);
                }
            };
            let package_generation = next_native_package_generation();
            let metadata = NativePluginHostMetadata {
                package_id: mod_id.to_owned(),
                package: package.clone(),
                package_generation,
                is_server,
                settings_json: settings_json.clone(),
                services: services.clone(),
            };
            let instance = match NativePluginInstance::load_with_mode(
                entry,
                metadata,
                NativePluginMode::ShroudForgeV2,
            ) {
                Ok(instance) => instance,
                Err(error) => {
                    retain_or_unload_new_library(library, bridge_initialized);
                    return Err(format!("V2 native plugin ABI negotiation failed: {error}"));
                }
            };
            let staged = StagedNativePlugin {
                mod_id: mod_id.to_owned(),
                entry: plugin.entry,
                path,
                package_directory,
                package_root,
                declaration: declaration.metadata,
                package,
                settings_json,
                package_generation,
                bridge_initialized,
                library,
                instance,
            };
            plugins.push(staged);
        }
        Ok(StagedNativePluginBatch {
            root,
            is_server,
            services,
            plugins: std::mem::take(&mut plugins),
        })
    })();

    match stage_result {
        Ok(batch) => SELECTED_NATIVE_PLUGINS.with(|state| {
            let mut state = state
                .try_borrow_mut()
                .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
            if !matches!(*state, SelectedNativePluginState::Loading) {
                cleanup_staged_batch(batch, None);
                *state = SelectedNativePluginState::Failed;
                return Err("Selected native plugin staging state changed unexpectedly".into());
            }
            *state = SelectedNativePluginState::Loaded(batch);
            Ok(())
        }),
        Err(error) => {
            for mut plugin in plugins.into_iter().rev() {
                let outcome = plugin.instance.rollback_preparation();
                release_staged_plugin(plugin, Some(outcome));
            }
            SELECTED_NATIVE_PLUGINS.with(|state| {
                if let Ok(mut state) = state.try_borrow_mut() {
                    *state = SelectedNativePluginState::Failed;
                }
            });
            Err(error)
        }
    }
}

/// Runs after the bootstrap factory negotiation and within the trusted held
/// entry operation, before framework service preparation starts.
pub(super) fn stage_selected_native_plugins_before_entry(
    root: &std::path::Path,
    is_server: bool,
) -> Result<(), String> {
    let batch: Result<StagedNativePluginBatch, String> = SELECTED_NATIVE_PLUGINS.with(|state| {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
        let previous = std::mem::replace(&mut *state, SelectedNativePluginState::Staging);
        match previous {
            SelectedNativePluginState::Loaded(batch)
                if selected_batch_matches(&batch, root, is_server) =>
            {
                Ok(batch)
            }
            previous => {
                *state = previous;
                Err("No matching loaded V2 native plugins are available for held staging".into())
            }
        }
    });
    let batch = batch?;
    let mut batch = batch;
    for index in 0..batch.plugins.len() {
        if let Err(failure) = batch.plugins[index].instance.stage_before_entry() {
            let detail = format!(
                "V2 native plugin {} failed pre-entry staging with status {}; {}",
                batch.plugins[index].mod_id,
                failure.status,
                failure.cleanup.description(),
            );
            let failed_batch = StagedNativePluginBatch {
                root: batch.root.clone(),
                is_server: batch.is_server,
                services: batch.services.clone(),
                plugins: std::mem::take(&mut batch.plugins),
            };
            cleanup_staged_batch(failed_batch, Some((index, failure.cleanup)));
            SELECTED_NATIVE_PLUGINS.with(|state| {
                if let Ok(mut state) = state.try_borrow_mut() {
                    *state = SelectedNativePluginState::Failed;
                }
            });
            return Err(detail);
        }
    }
    let mut batch = Some(batch);
    let staged = SELECTED_NATIVE_PLUGINS.with(|state| {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
        if !matches!(*state, SelectedNativePluginState::Staging) {
            *state = SelectedNativePluginState::Failed;
            return Err("Selected native plugin staging state changed unexpectedly".into());
        }
        *state = SelectedNativePluginState::Staged(
            batch.take().expect("staged native batch is present"),
        );
        Ok(())
    });
    if let Err(error) = staged {
        if let Some(batch) = batch.take() {
            cleanup_staged_batch(batch, None);
        }
        return Err(error);
    }
    Ok(())
}

fn retain_or_unload_new_library(mut library: NativeLibrary, bridge_initialized: bool) {
    if bridge_initialized {
        std::mem::forget(library);
    } else if let Err(error) = library.try_free() {
        tracing::error!(target: "shroudforge::runtime", %error,
            "Retaining an uninitialized native plugin module after unload failure");
        std::mem::forget(library);
    }
}

pub(super) fn prepare_selected_native_plugins(
    root: &std::path::Path,
    is_server: bool,
) -> Result<(), String> {
    let batch: Result<StagedNativePluginBatch, String> = SELECTED_NATIVE_PLUGINS.with(|state| {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
        let previous = std::mem::replace(&mut *state, SelectedNativePluginState::Preparing);
        match previous {
            SelectedNativePluginState::Staged(batch)
                if selected_batch_matches(&batch, root, is_server) =>
            {
                Ok(batch)
            }
            previous => {
                *state = previous;
                Err(
                    "No matching staged V2 native plugins are available for held preparation"
                        .into(),
                )
            }
        }
    });
    let batch = batch?;
    let mut batch = batch;
    for index in 0..batch.plugins.len() {
        match batch.plugins[index].instance.prepare_before_entry() {
            Ok(()) => {}
            Err(failure) => {
                let detail = format!(
                    "V2 native plugin {} failed pre-entry preparation with status {}; {}",
                    batch.plugins[index].mod_id,
                    failure.status,
                    failure.cleanup.description(),
                );
                let failed_batch = StagedNativePluginBatch {
                    root: batch.root.clone(),
                    is_server: batch.is_server,
                    services: batch.services.clone(),
                    plugins: std::mem::take(&mut batch.plugins),
                };
                cleanup_staged_batch(failed_batch, Some((index, failure.cleanup)));
                SELECTED_NATIVE_PLUGINS.with(|state| {
                    if let Ok(mut state) = state.try_borrow_mut() {
                        *state = SelectedNativePluginState::Failed;
                    }
                });
                return Err(detail);
            }
        }
    }
    if let Err(error) = publish_entry_restored_continuations(&batch) {
        cleanup_staged_batch(batch, None);
        SELECTED_NATIVE_PLUGINS.with(|state| {
            if let Ok(mut state) = state.try_borrow_mut() {
                *state = SelectedNativePluginState::Failed;
            }
        });
        return Err(format!(
            "Could not publish selected V2 entry continuation: {error}"
        ));
    }
    let mut batch = Some(batch);
    let prepared = SELECTED_NATIVE_PLUGINS.with(|state| {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
        if !matches!(*state, SelectedNativePluginState::Preparing) {
            *state = SelectedNativePluginState::Failed;
            return Err("Selected native plugin preparation state changed unexpectedly".into());
        }
        *state = SelectedNativePluginState::Prepared(
            batch.take().expect("prepared native batch is present"),
        );
        Ok(())
    });
    if let Err(error) = prepared {
        if let Some(batch) = batch.take() {
            cleanup_staged_batch(batch, None);
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn selected_native_plugin_services(
    root: &std::path::Path,
    is_server: bool,
) -> Result<Option<Arc<dyn crate::session::native_plugin_abi::NativePluginServiceLifetime>>, String>
{
    let root = canonical_native_path(root)?;
    SELECTED_NATIVE_PLUGINS.with(|state| {
        let state = state
            .try_borrow()
            .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
        match &*state {
            SelectedNativePluginState::Empty => Ok(None),
            SelectedNativePluginState::Prepared(batch)
                if same_native_path(&batch.root, &root) && batch.is_server == is_server =>
            {
                Ok(Some(batch.services.clone()))
            }
            SelectedNativePluginState::Adopted {
                root: staged_root,
                is_server: staged_server,
            } if same_native_path(staged_root, &root) && *staged_server == is_server => Ok(None),
            _ => Err(
                "Selected V2 native plugin services do not match the runtime package root or role"
                    .into(),
            ),
        }
    })
}

pub(super) fn selected_native_plugins_adopted(root: &std::path::Path, is_server: bool) -> bool {
    let Ok(root) = canonical_native_path(root) else {
        return false;
    };
    SELECTED_NATIVE_PLUGINS.with(|state| {
        state.try_borrow().is_ok_and(|state| {
            matches!(&*state, SelectedNativePluginState::Adopted { root: staged_root, is_server: staged_server }
                if same_native_path(staged_root, &root) && *staged_server == is_server)
        })
    })
}

pub(super) fn discard_staged_native_plugins(root: &std::path::Path, is_server: bool) {
    let Ok(root) = canonical_native_path(root) else {
        return;
    };
    let batch = SELECTED_NATIVE_PLUGINS.with(|state| {
        let Ok(mut state) = state.try_borrow_mut() else {
            return None;
        };
        let previous = std::mem::replace(&mut *state, SelectedNativePluginState::Failed);
        match previous {
            SelectedNativePluginState::Loaded(batch)
                if same_native_path(&batch.root, &root) && batch.is_server == is_server =>
            {
                Some(batch)
            }
            SelectedNativePluginState::Staged(batch)
                if same_native_path(&batch.root, &root) && batch.is_server == is_server =>
            {
                Some(batch)
            }
            SelectedNativePluginState::Prepared(batch)
                if same_native_path(&batch.root, &root) && batch.is_server == is_server =>
            {
                Some(batch)
            }
            previous => {
                *state = previous;
                None
            }
        }
    });
    if let Some(batch) = batch {
        cleanup_staged_batch(batch, None);
    }
}

fn take_prepared_native_plugins(
    root: &std::path::Path,
    is_server: bool,
) -> Result<Option<StagedNativePluginBatch>, String> {
    let root = canonical_native_path(root)?;
    SELECTED_NATIVE_PLUGINS.with(|state| {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| "Selected native plugin state is already in use".to_owned())?;
        let previous = std::mem::replace(&mut *state, SelectedNativePluginState::Adopting);
        match previous {
            SelectedNativePluginState::Empty => {
                *state = SelectedNativePluginState::Empty;
                Ok(None)
            }
            SelectedNativePluginState::Prepared(batch)
                if same_native_path(&batch.root, &root) && batch.is_server == is_server =>
            {
                Ok(Some(batch))
            }
            previous => {
                *state = previous;
                Err("Selected V2 native plugin batch is not prepared for this runtime owner".into())
            }
        }
    })
}

fn set_selected_native_plugins_failed() {
    SELECTED_NATIVE_PLUGINS.with(|state| {
        if let Ok(mut state) = state.try_borrow_mut() {
            *state = SelectedNativePluginState::Failed;
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeNativePluginDeclaration {
    metadata: mod_loader::dll_manager::PluginIniDeclaration,
}

fn read_native_plugin_declaration(
    target_mod: &Mod,
) -> Result<Option<RuntimeNativePluginDeclaration>, String> {
    let config_path = camino::Utf8Path::new("native-plugin.ini");
    let mut filesystem = target_mod.fs();
    if !filesystem.is_file(config_path) {
        return Ok(None);
    }
    let mut config = String::new();
    filesystem
        .read_file(config_path)
        .map_err(|error| format!("could not read native-plugin.ini: {error}"))?
        .read_to_string(&mut config)
        .map_err(|error| format!("could not decode native-plugin.ini: {error}"))?;
    let metadata = mod_loader::dll_manager::parse_plugin_ini(&config);
    Ok(Some(RuntimeNativePluginDeclaration { metadata }))
}

fn native_plugin_mode(
    metadata: &mod_loader::dll_manager::PluginIniDeclaration,
) -> Result<NativePluginMode, String> {
    match metadata.state {
        mod_loader::dll_manager::PluginIniState::Missing => Ok(NativePluginMode::LegacyLoadOnly),
        mod_loader::dll_manager::PluginIniState::Invalid => {
            Err("native-plugin.ini has an invalid [Plugin] declaration".into())
        }
        mod_loader::dll_manager::PluginIniState::Disabled
        | mod_loader::dll_manager::PluginIniState::Enabled => match metadata.lifecycle.as_deref() {
            None => Ok(NativePluginMode::LegacyLoadOnly),
            Some(lifecycle) if lifecycle.eq_ignore_ascii_case("shroudforge-v1") => {
                Ok(NativePluginMode::ShroudForgeV1)
            }
            Some(lifecycle) if lifecycle.eq_ignore_ascii_case("shroudforge-v2") => {
                Ok(NativePluginMode::ShroudForgeV2)
            }
            Some(lifecycle) => Err(format!("unsupported native plugin lifecycle '{lifecycle}'")),
        },
    }
}
impl NativePluginMode {
    fn is_lifecycle_managed(self) -> bool {
        matches!(self, Self::ShroudForgeV1 | Self::ShroudForgeV2)
    }
}
fn normalize_plugin_path(path: &str) -> String {
    path.trim_start_matches("./")
        .replace('\\', "/")
        .to_ascii_lowercase()
}

pub(crate) fn native_overlay_startup_demand(environment: &ModEnvironment) -> Result<bool, String> {
    for plugin in
        crate::native_dlls::deferred_native_plugins(environment.game_dir().as_std_path(), false)?
    {
        if !plugin.declaration.state.is_enabled()
            || !plugin.declaration.declares_service("shroudforge.overlay")
        {
            continue;
        }
        let Some(mod_id) = plugin.entry.mod_id.as_deref() else {
            continue;
        };
        let Some(package) = environment.mod_registry().get(mod_id) else {
            continue;
        };
        let info = package.info();
        if info.enabled
            && info.target.supports_process(false)
            && info.capabilities.contains(&Capability::RuntimeRegisterDll)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
impl AppState {
    /// Install the runtime-owned service gate before opted-in plugins are
    /// initialized. Render callbacks must use this gate without touching Lua
    /// or the AppState RefCell.
    pub(crate) fn set_native_plugin_service_lifetime(
        &self,
        services: std::sync::Arc<
            dyn crate::session::native_plugin_abi::NativePluginServiceLifetime,
        >,
    ) -> Result<(), String> {
        if self
            .native_dlls
            .borrow()
            .iter()
            .any(NativeDll::is_lifecycle_managed)
            || self
                .pending_native_dlls
                .borrow()
                .iter()
                .any(|item| item.mode.is_lifecycle_managed())
        {
            return Err(
                "native plugin service lifetime must be installed before plugin initialization"
                    .into(),
            );
        }
        *self.native_plugin_services.borrow_mut() = services;
        Ok(())
    }

    pub(crate) fn adopt_selected_native_plugins(&self) -> Result<(), String> {
        let root = self.env.game_dir().as_std_path();
        let is_server = self.is_server();
        let Some(batch) = take_prepared_native_plugins(root, is_server)? else {
            return Ok(());
        };
        if let Err(error) = self.validate_staged_native_plugin_batch(&batch) {
            cleanup_staged_batch(batch, None);
            set_selected_native_plugins_failed();
            return Err(error);
        }

        let StagedNativePluginBatch {
            root: staged_root,
            is_server: staged_server,
            services: _,
            plugins,
        } = batch;
        let mut active = Vec::new();
        let mut pending = plugins.into_iter();
        while let Some(mut plugin) = pending.next() {
            match plugin.instance.start() {
                Ok(()) => active.push(plugin),
                Err(failure) => {
                    let detail = format!(
                        "V2 native plugin {} failed initialization with status {}; {}",
                        plugin.mod_id,
                        failure.status,
                        failure.cleanup.description(),
                    );
                    release_staged_plugin(plugin, Some(failure.cleanup));
                    for mut active_plugin in active.into_iter().rev() {
                        let outcome = active_plugin.instance.stop();
                        release_staged_plugin(active_plugin, Some(outcome));
                    }
                    for mut pending_plugin in pending {
                        let outcome = pending_plugin.instance.rollback_preparation();
                        release_staged_plugin(pending_plugin, Some(outcome));
                    }
                    set_selected_native_plugins_failed();
                    return Err(detail);
                }
            }
        }

        let adopted: Vec<_> = active
            .into_iter()
            .map(|plugin| NativeDll {
                mod_id: plugin.mod_id,
                path: plugin.path,
                mode: NativePluginMode::ShroudForgeV2,
                bridge_initialized: plugin.bridge_initialized,
                library: plugin.library,
                module: NativeModule::Lifecycle(Box::new(plugin.instance)),
            })
            .collect();
        self.native_dlls.borrow_mut().extend(adopted);
        SELECTED_NATIVE_PLUGINS.with(|state| {
            if let Ok(mut state) = state.try_borrow_mut() {
                *state = SelectedNativePluginState::Adopted {
                    root: staged_root,
                    is_server: staged_server,
                };
            }
        });
        let adopted_ids: Vec<_> = self
            .native_dlls
            .borrow()
            .iter()
            .filter(|plugin| {
                plugin.mode() == NativePluginMode::ShroudForgeV2
                    && plugin.is_active()
                    && plugin.path.starts_with(root)
            })
            .map(|plugin| plugin.mod_id.clone())
            .collect();
        for mod_id in adopted_ids {
            self.report_runtime_effect(
                &mod_id,
                "loaded",
                "ShroudForge V2 native plugin was prepared before entry and adopted into the runtime lifecycle",
            );
        }
        Ok(())
    }

    fn validate_staged_native_plugin_batch(
        &self,
        batch: &StagedNativePluginBatch,
    ) -> Result<(), String> {
        if !selected_batch_matches(batch, self.env.game_dir().as_std_path(), self.is_server()) {
            return Err("Staged V2 native plugin root or process role differs from runtime".into());
        }
        let app_services = self.native_plugin_services.borrow().clone();
        if !Arc::ptr_eq(&app_services, &batch.services) {
            return Err(
                "V2 native plugin service owner changed between preparation and adoption".into(),
            );
        }
        let selected = crate::native_dlls::deferred_native_plugins(
            self.env.game_dir().as_std_path(),
            self.is_server(),
        )?;
        let selected_v2: Vec<_> = selected
            .iter()
            .filter(|plugin| {
                plugin.declaration.state.is_enabled()
                    && plugin
                        .declaration
                        .lifecycle
                        .as_deref()
                        .is_some_and(|lifecycle| lifecycle.eq_ignore_ascii_case("shroudforge-v2"))
            })
            .collect();
        if selected_v2.len() != batch.plugins.len() {
            return Err("Selected V2 native plugin count changed before runtime adoption".into());
        }
        let mut package_ids = std::collections::HashSet::new();
        let mut dll_paths = std::collections::HashSet::new();
        for (selected, staged) in selected_v2.into_iter().zip(&batch.plugins) {
            if !package_ids.insert(staged.mod_id.as_str())
                || !dll_paths.insert(staged.path.clone())
                || selected.entry.mod_id.as_deref() != Some(staged.mod_id.as_str())
                || selected.entry.package_directory != staged.package_directory
                || selected.entry.mode != DllLoadMode::Library
                || !selected.declaration.eq(&staged.declaration)
                || !same_native_path(&selected.entry.path, &staged.path)
            {
                return Err("Selected V2 native plugin identity changed before adoption".into());
            }
            let authorized = crate::native_dlls::authorize_deferred_plugin(
                self.env.game_dir().as_std_path(),
                self.is_server(),
                &staged.entry,
            )?;
            if authorized.declaration != staged.declaration
                || !same_native_path(&authorized.entry.path, &staged.path)
            {
                return Err("V2 plugin no longer matches DLL Manager's selected entry".into());
            }
            let package = self
                .env
                .mod_registry()
                .get(&staged.mod_id)
                .ok_or("V2 native plugin is unavailable in the runtime package registry")?;
            let package_root = canonical_native_path(package.fs().root().as_std_path())?;
            if package.info().id != staged.mod_id
                || !package.info().enabled
                || !package.info().target.supports_process(self.is_server())
                || !package
                    .info()
                    .capabilities
                    .contains(&Capability::RuntimeRegisterDll)
                || !same_native_path(&package_root, &staged.package_root)
                || staged.package.info().id != staged.mod_id
                || !same_native_path(
                    &canonical_native_path(staged.package.fs().root().as_std_path())?,
                    &staged.package_root,
                )
            {
                return Err("V2 native plugin package owner changed before adoption".into());
            }
            let manifest = mod_loader::config::read_manifest_path(
                self.env.game_dir().as_std_path(),
                &package_root,
            )
            .map_err(|error| format!("Could not revalidate V2 package at adoption: {error}"))?;
            if manifest.id != staged.mod_id
                || !manifest.enabled
                || !manifest.target.supports_process(self.is_server())
                || !manifest
                    .capabilities
                    .contains(&Capability::RuntimeRegisterDll)
            {
                return Err("V2 package admission changed before runtime adoption".into());
            }
            let settings_json = serde_json::to_vec(&package.info().setting_values)
                .map_err(|error| format!("Could not encode current V2 settings: {error}"))?;
            if settings_json != staged.settings_json
                || staged.instance.mode() != NativePluginMode::ShroudForgeV2
                || staged.instance.package_generation() != staged.package_generation
                || !Arc::ptr_eq(&staged.instance.service_lifetime(), &batch.services)
            {
                return Err(
                    "V2 plugin host context, settings, or generation changed before adoption"
                        .into(),
                );
            }
        }
        if self.native_dlls.borrow().iter().any(|dll| {
            batch
                .plugins
                .iter()
                .any(|plugin| dll.mod_id == plugin.mod_id)
        }) || self.pending_native_dlls.borrow().iter().any(|pending| {
            batch
                .plugins
                .iter()
                .any(|plugin| pending.mod_id == plugin.mod_id)
        }) {
            return Err("A selected V2 native plugin is already loaded or pending".into());
        }
        Ok(())
    }

    pub(super) fn native_dll_state(&self, id: &str) -> Option<&'static str> {
        if self
            .native_dlls
            .borrow()
            .iter()
            .any(|dll| dll.mod_id == id && dll.is_retained())
        {
            Some("retained")
        } else if self
            .native_dlls
            .borrow()
            .iter()
            .any(|dll| dll.mod_id == id && matches!(&dll.module, NativeModule::StoppedBridge))
        {
            Some("stopped")
        } else if self.native_dlls.borrow().iter().any(|dll| dll.mod_id == id) {
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

    pub(super) fn native_dll_status(
        &self,
        id: &str,
    ) -> Option<crate::native_dlls::NativeDllStatus> {
        crate::native_dlls::statuses()
            .into_iter()
            .find(|status| status.id == id)
    }

    pub(super) fn report_native_dll_effect(&self, status: &crate::native_dlls::NativeDllStatus) {
        self.report_runtime_effect(&status.id, status.state, &status.detail);
        if let Some(effect) = self.runtime_effects.borrow_mut().get_mut(&status.id) {
            effect["nativeSource"] = status.source.clone().into();
        }
    }

    pub(crate) fn load_native_dll_declaration(
        &self,
        target_mod: &Mod,
        authorized: &crate::native_dlls::DeferredNativePlugin,
    ) -> Result<bool, String> {
        let root = self.env.game_dir().as_std_path();
        let current = crate::native_dlls::authorize_deferred_plugin(
            root,
            self.is_server(),
            &authorized.entry,
        )?;
        if current.declaration != authorized.declaration {
            return Err(
                "Lifecycle plugin declaration changed after DLL Manager authorization".into(),
            );
        }
        if authorized.entry.mod_id.as_deref() != Some(target_mod.info().id.as_str()) {
            return Err(
                "DLL Manager lifecycle selection does not match the runtime package".into(),
            );
        }
        let package_directory = target_mod
            .fs()
            .root()
            .file_name()
            .unwrap_or_default()
            .to_owned();
        if !package_directory.eq_ignore_ascii_case(&authorized.entry.package_directory) {
            return Err("DLL Manager lifecycle selection does not match the package folder".into());
        }
        if !self.has_feature(AppFeatures::RUNTIME_DLL) {
            return Err(
                "ShroudForge native plugin lifecycle is unavailable in this execution phase".into(),
            );
        }
        let Some(declaration) = read_native_plugin_declaration(target_mod)? else {
            return Err("Authorized lifecycle plugin no longer has native-plugin.ini".into());
        };
        if declaration.metadata != authorized.declaration {
            return Err("native-plugin.ini changed after DLL Manager authorization".into());
        }
        if !declaration.metadata.state.is_enabled() {
            self.report_runtime_effect(
                &target_mod.info().id,
                "disabled",
                "ShroudForge native plugin is disabled in native-plugin.ini",
            );
            return Ok(false);
        }
        let mode = native_plugin_mode(&declaration.metadata)?;
        if !mode.is_lifecycle_managed() {
            return Err(
                "Authorized native plugin no longer declares a ShroudForge lifecycle".into(),
            );
        }
        self.validate_native_plugin_package(target_mod)?;
        let relative_path = declaration
            .metadata
            .dll
            .as_deref()
            .ok_or_else(|| "native-plugin.ini does not name a DLL".to_owned())?;
        let selected = mod_loader::dll_manager::resolve_package_dll_path(
            root,
            &package_directory,
            &mod_loader::dll_manager::DllProfileFile {
                path: relative_path.to_owned(),
                mode: DllLoadMode::Library,
            },
        )?;
        if selected != authorized.entry.path {
            return Err("DLL Manager lifecycle path no longer matches native-plugin.ini".into());
        }
        if mode == NativePluginMode::ShroudForgeV2 {
            if let Some(dll) = self.native_dlls.borrow().iter().find(|dll| {
                dll.mod_id == target_mod.info().id && dll.mode() == NativePluginMode::ShroudForgeV2
            }) {
                if dll.is_active() {
                    return Ok(true);
                }
                return Err(
                    "V2 native plugin is retained or stopped; process restart is required".into(),
                );
            }
            return Err("V2 native plugin was not adopted from pre-entry startup".into());
        }
        self.queue_mod_native_dll(target_mod, relative_path, mode)?;
        Ok(true)
    }

    pub(crate) fn restart_native_plugin(&self, mod_id: &str) -> Result<(), String> {
        let plugins = crate::native_dlls::deferred_native_plugins(
            self.env.game_dir().as_std_path(),
            self.is_server(),
        )?;
        let Some(plugin) = plugins
            .into_iter()
            .find(|plugin| plugin.entry.mod_id.as_deref() == Some(mod_id))
        else {
            return Ok(());
        };
        if plugin
            .declaration
            .lifecycle
            .as_deref()
            .is_some_and(|lifecycle| lifecycle.eq_ignore_ascii_case("shroudforge-v2"))
        {
            return Err(
                "V2 native plugins are process-start only and cannot be restarted in place".into(),
            );
        }
        let Some(target_mod) = self.env.mod_registry().get(mod_id) else {
            return Ok(());
        };
        let _ = self.load_native_dll_declaration(target_mod, &plugin)?;
        Ok(())
    }

    pub(super) fn validate_native_plugin_package(&self, target_mod: &Mod) -> Result<(), String> {
        if !self.has_feature(AppFeatures::RUNTIME_DLL) {
            return Err(
                "ShroudForge native plugin lifecycle is unavailable in this execution phase".into(),
            );
        }
        if !target_mod
            .info()
            .capabilities
            .contains(&Capability::RuntimeRegisterDll)
        {
            return Err(
                "ShroudForge native plugins require runtime-register-dll capability".into(),
            );
        }
        if !target_mod.info().target.supports_process(self.is_server()) {
            return Err(format!(
                "native plugin package target does not include the {} process",
                if self.is_server() { "server" } else { "client" }
            ));
        }
        Ok(())
    }

    /// Lua registrations retain their EML execution phase and package boundary.
    /// Lifecycle plugins must also be in the exact current DLL Manager plan.
    pub(crate) fn load_mod_native_dll(
        &self,
        target_mod: &Mod,
        relative_path: &str,
    ) -> Result<(), String> {
        if !target_mod.info().enabled {
            self.report_runtime_effect(
                &target_mod.info().id,
                "restart-required",
                "Native DLL was skipped because the mod is disabled. Enable it and restart the game",
            );
            return Ok(());
        }
        if !self.has_feature(AppFeatures::RUNTIME_DLL) {
            return Err("native DLL loading is unavailable in this execution phase".into());
        }
        let package_directory = target_mod
            .fs()
            .root()
            .file_name()
            .unwrap_or_default()
            .to_owned();
        if mod_loader::dll_manager::read_selections(self.env.game_dir().as_std_path())
            .iter()
            .find(|selection| {
                selection
                    .package_directory
                    .eq_ignore_ascii_case(&package_directory)
            })
            .is_some_and(|selection| !selection.target.applies_to(self.is_server()))
        {
            return Err("The DLL Manager selection targets a different game process".into());
        }
        let relative_path = normalize_plugin_path(relative_path);
        let metadata = read_native_plugin_declaration(target_mod)?;
        let selected_is_declared = metadata.as_ref().is_some_and(|declaration| {
            declaration
                .metadata
                .declared_dlls
                .iter()
                .any(|dll| normalize_plugin_path(dll) == relative_path)
        });
        let disposition = match metadata.as_ref() {
            Some(declaration) => mod_loader::dll_manager::classify_plugin_load(
                &declaration.metadata,
                selected_is_declared,
            )?,
            None => PluginLoadDisposition::Ordinary,
        };
        let selected = mod_loader::dll_manager::resolve_package_dll_path(
            self.env.game_dir().as_std_path(),
            &package_directory,
            &mod_loader::dll_manager::DllProfileFile {
                path: relative_path.clone(),
                mode: DllLoadMode::Library,
            },
        )?;
        let mod_id = target_mod.info().id.clone();
        let status_id = format!(
            "dll:{}:{}",
            package_directory.to_ascii_lowercase(),
            relative_path.to_ascii_lowercase()
        );
        let entry = mod_loader::dll_manager::DllLoadEntry {
            status_id,
            mod_id: Some(mod_id),
            package_directory: package_directory.clone(),
            path: selected,
            mode: DllLoadMode::Library,
            source: "Lua register_dll".into(),
        };
        match disposition {
            PluginLoadDisposition::ShroudForgeV1 | PluginLoadDisposition::ShroudForgeV2 => {
                let declaration = metadata.expect("managed declaration was parsed");
                if !declaration.metadata.state.is_enabled() {
                    self.report_runtime_effect(
                        &target_mod.info().id,
                        "disabled",
                        "ShroudForge native plugin is disabled in native-plugin.ini",
                    );
                    return Ok(());
                }
                let mode = native_plugin_mode(&declaration.metadata)?;
                if !mode.is_lifecycle_managed() {
                    return Err("Unsupported native plugin lifecycle cannot be loaded".into());
                }
                self.validate_native_plugin_package(target_mod)?;
                let authorized = crate::native_dlls::authorize_deferred_plugin(
                    self.env.game_dir().as_std_path(),
                    self.is_server(),
                    &entry,
                )?;
                if declaration.metadata != authorized.declaration {
                    return Err("native-plugin.ini changed after DLL Manager authorization".into());
                }
                if mode == NativePluginMode::ShroudForgeV2 {
                    if self
                        .native_dlls
                        .borrow()
                        .iter()
                        .any(|dll| dll.mod_id == target_mod.info().id && dll.mode() == mode)
                    {
                        return Ok(());
                    }
                    return Err("V2 native plugin must be staged by selected startup".into());
                }
                self.queue_mod_native_dll(target_mod, &relative_path, mode)?;
            }
            PluginLoadDisposition::Ordinary => {
                crate::native_dlls::queue(self.env.game_dir().as_std_path(), entry)?;
            }
        }
        self.poll_native_dll_loads();
        Ok(())
    }
    pub(super) fn queue_mod_native_dll(
        &self,
        target_mod: &Mod,
        relative_path: &str,
        mode: NativePluginMode,
    ) -> Result<(), String> {
        if let Some(declaration) = read_native_plugin_declaration(target_mod)?
            && declaration
                .metadata
                .lifecycle
                .as_deref()
                .is_some_and(|lifecycle| lifecycle.eq_ignore_ascii_case("shroudforge-v2"))
        {
            let declared_path = declaration
                .metadata
                .dll
                .as_deref()
                .map(normalize_plugin_path);
            let requested_path = normalize_plugin_path(relative_path);
            if declared_path.as_deref() != Some(requested_path.as_str()) {
                return Err(
                    "A V2 native plugin declaration owns this package's DLLs; other DLLs cannot be queued"
                        .into(),
                );
            }
            if mode != NativePluginMode::ShroudForgeV2 {
                return Err(
                    "A V2 native plugin DLL cannot be queued through an ordinary or V1 load path"
                        .into(),
                );
            }
            return Err("V2 native plugins are loaded only through selected startup".into());
        }
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
        let is_duplicate = {
            let loaded = self.native_dlls.borrow();
            let pending = self.pending_native_dlls.borrow();
            loaded.iter().any(|dll| {
                is_duplicate_request(
                    mode,
                    &mod_id,
                    &canonical,
                    dll.mode(),
                    &dll.mod_id,
                    &dll.path,
                )
            }) || pending.iter().any(|dll| {
                is_duplicate_request(mode, &mod_id, &canonical, dll.mode, &dll.mod_id, &dll.path)
            })
        };
        if is_duplicate {
            if let Some(directory) = temporary_directory {
                let _ = std::fs::remove_dir_all(directory);
            }
            return Ok(());
        }

        let settings_json = serde_json::to_vec(&target_mod.info().setting_values)
            .map_err(|error| format!("could not encode native plugin settings: {error}"))?;
        if settings_json.len() > crate::session::native_plugin_abi::NATIVE_MAX_SETTINGS_JSON_BYTES {
            if let Some(directory) = temporary_directory {
                let _ = std::fs::remove_dir_all(directory);
            }
            return Err("native plugin settings JSON exceeds 65536 bytes".into());
        }
        let package_generation = next_native_package_generation();
        let (sender, receiver) = mpsc::channel();
        let worker_mod_id = mod_id.clone();
        let worker_path = canonical.clone();
        let worker_tempdir = temporary_directory.clone();
        let worker_package_directory =
            shroudforge_loader_config::paths::mods_dir(self.env.game_dir().as_std_path())
                .join(target_mod.fs().root().file_name().unwrap_or_default());
        tracing::debug!(target: "shroudforge::runtime", mod_id, dll = %canonical.display(), lifecycle = ?mode,
            "Queuing native DLL load and optional bridge initialization on isolated worker");
        spawn_retained_native_worker(
            format!(
                "sf-native-{}",
                mod_id.chars().take(24).collect::<String>()
            ),
            crate::native_dlls::pin_native_runtime_module,
            move || {
                tracing::debug!(target: "shroudforge::runtime", mod_id = %worker_mod_id,
                    dll = %worker_path.display(), "Isolated native DLL load started");
                let result = load_native_library_with_bridge(
                    &worker_mod_id,
                    &worker_path,
                    worker_tempdir.clone(),
                    &worker_package_directory,
                );
                match &result {
                    Ok(_) => {
                        tracing::debug!(target: "shroudforge::runtime", mod_id = %worker_mod_id,
                            dll = %worker_path.display(), "Isolated native DLL load completed")
                    }
                    Err(NativeLibraryLoadError::Load(error)) => {
                        if let Some(directory) = worker_tempdir {
                            let _ = std::fs::remove_dir_all(directory);
                        }
                        tracing::error!(target: "shroudforge::runtime", mod_id = %worker_mod_id,
                            dll = %worker_path.display(), error, "Isolated native DLL load failed")
                    }
                    Err(NativeLibraryLoadError::Bridge { detail, .. }) => {
                        tracing::error!(target: "shroudforge::runtime", mod_id = %worker_mod_id,
                            dll = %worker_path.display(), error = %detail, "Native bridge initialization failed; module code will remain mapped")
                    }
                }
                // A bridge host and callbacks are process-owned. Preserve the
                // module whenever the receiver disappears after those pointers
                // could have escaped.
                if let Err(error) = sender.send(result) {
                    match error.0 {
                        Ok(loaded) if loaded.bridge_initialized => {
                            std::mem::forget(loaded.library);
                        }
                        Err(NativeLibraryLoadError::Bridge { library, .. }) => {
                            std::mem::forget(library);
                        }
                        _ => {}
                    }
                }
            },
        )
            .map_err(|error| {
                if let Some(directory) = &temporary_directory {
                    let _ = std::fs::remove_dir_all(directory);
                }
                error
            })?;
        self.pending_native_dlls
            .borrow_mut()
            .push(PendingNativeDll {
                mod_id,
                path: canonical,
                package_relative_path: relative_path.to_owned(),
                mode,
                package: target_mod.clone(),
                package_generation,
                settings_json,
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
                            tracing::warn!(target: "shroudforge::runtime", mod_id = %pending[index].mod_id,
                                dll = %pending[index].path.display(), elapsed_ms = pending[index].queued_at.elapsed().as_millis(),
                                "Native DLL load is still running on an isolated worker. Runtime updates continue");
                            pending[index].warned_stall = true;
                        }
                        index += 1;
                    }
                }
            }
        }
        for (item, result) in completed {
            let result = result.unwrap_or_else(|| {
                Err(NativeLibraryLoadError::Load(
                    "native DLL loader worker exited without a result".into(),
                ))
            });
            match result {
                Ok(library) => self.finish_native_library_load(item, library),
                Err(NativeLibraryLoadError::Load(error)) => {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %item.mod_id,
                        dll = %item.path.display(), "Native DLL load failed: {error}");
                    self.report_runtime_effect(&item.mod_id, "error", &error);
                }
                Err(NativeLibraryLoadError::Bridge { library, detail }) => {
                    self.retain_bridge_library(
                        item.mod_id.clone(),
                        item.path.clone(),
                        item.mode,
                        library,
                    );
                    self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        &format!("{detail}; the native module is retained until process exit"),
                    );
                }
            }
        }
        for status in crate::native_dlls::statuses() {
            self.report_native_dll_effect(&status);
        }
    }

    fn finish_native_library_load(&self, item: PendingNativeDll, loaded: LoadedNativeLibrary) {
        let LoadedNativeLibrary {
            mut library,
            bridge_initialized,
        } = loaded;
        if item.mode == NativePluginMode::LegacyLoadOnly {
            self.native_dlls.borrow_mut().push(NativeDll {
                mod_id: item.mod_id.clone(),
                path: item.path.clone(),
                mode: item.mode,
                bridge_initialized,
                library,
                module: NativeModule::LegacyLoadOnly,
            });
            tracing::info!(target: "shroudforge::runtime", mod_id = %item.mod_id,
                dll = %item.path.display(), "Legacy native DLL load completed without lifecycle initialization");
            self.report_runtime_effect(
                &item.mod_id,
                "loaded",
                "legacy native DLL loaded; lifecycle callbacks were not negotiated",
            );
            return;
        }

        match self.pending_plugin_is_current(&item) {
            Ok(true) => {}
            Ok(false) => {
                if bridge_initialized {
                    self.retain_bridge_library(
                        item.mod_id.clone(),
                        item.path.clone(),
                        item.mode,
                        library,
                    );
                    self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        "native bridge initialized before package selection changed; module code remains mapped until process exit",
                    );
                } else if let Err(error) = self.release_uninitialized_library(library) {
                    self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        &format!("stale native plugin load could not be released: {error}"),
                    );
                } else {
                    self.report_runtime_effect(
                        &item.mod_id,
                        "stopped",
                        "native plugin was disabled or changed before load completion",
                    );
                }
                return;
            }
            Err(error) => {
                if bridge_initialized {
                    self.retain_bridge_library(
                        item.mod_id.clone(),
                        item.path.clone(),
                        item.mode,
                        library,
                    );
                    self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        &format!(
                            "{error}; native bridge module code remains mapped until process exit"
                        ),
                    );
                } else if let Err(unload_error) = self.release_uninitialized_library(library) {
                    self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        &format!("{error}; {unload_error}"),
                    );
                } else {
                    self.report_runtime_effect(&item.mod_id, "error", &error);
                }
                return;
            }
        }

        let entry = match library.entrypoint(item.mode) {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                let error = "opt-in native plugin is missing ShroudForgePluginEntry";
                tracing::error!(target: "shroudforge::runtime", mod_id = %item.mod_id, "{error}");
                match self.release_or_retain_uninitialized_library(
                    &item.mod_id,
                    &item.path,
                    item.mode,
                    library,
                    bridge_initialized,
                ) {
                    Ok(()) => self.report_runtime_effect(&item.mod_id, "error", error),
                    Err(unload_error) => self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        &format!("{error}; {unload_error}"),
                    ),
                }
                return;
            }
            Err(error) => {
                match self.release_or_retain_uninitialized_library(
                    &item.mod_id,
                    &item.path,
                    item.mode,
                    library,
                    bridge_initialized,
                ) {
                    Ok(()) => self.report_runtime_effect(&item.mod_id, "error", &error),
                    Err(unload_error) => self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        &format!("{error}; {unload_error}"),
                    ),
                }
                return;
            }
        };
        let services = self.native_plugin_services.borrow().clone();
        let metadata = NativePluginHostMetadata {
            package_id: item.mod_id.clone(),
            package: item.package,
            package_generation: item.package_generation,
            is_server: self.is_server(),
            settings_json: item.settings_json,
            services,
        };
        let mut instance = match NativePluginInstance::load_with_mode(entry, metadata, item.mode) {
            Ok(instance) => instance,
            Err(error) => {
                tracing::error!(target: "shroudforge::runtime", mod_id = %item.mod_id, "Native plugin ABI negotiation failed: {error}");
                match self.release_or_retain_uninitialized_library(
                    &item.mod_id,
                    &item.path,
                    item.mode,
                    library,
                    bridge_initialized,
                ) {
                    Ok(()) => self.report_runtime_effect(&item.mod_id, "error", &error),
                    Err(unload_error) => self.report_runtime_effect(
                        &item.mod_id,
                        "retained",
                        &format!("{error}; {unload_error}"),
                    ),
                }
                return;
            }
        };

        match instance.start() {
            Ok(()) => {
                self.native_dlls.borrow_mut().push(NativeDll {
                    mod_id: item.mod_id.clone(),
                    path: item.path.clone(),
                    mode: item.mode,
                    bridge_initialized,
                    library,
                    module: NativeModule::Lifecycle(Box::new(instance)),
                });
                tracing::info!(target: "shroudforge::runtime", mod_id = %item.mod_id,
                    dll = %item.path.display(), "ShroudForge native plugin initialized on the runtime owner thread");
                self.report_runtime_effect(
                    &item.mod_id,
                    "loaded",
                    "ShroudForge native plugin lifecycle is active",
                );
            }
            Err(failure) => {
                let detail = format!(
                    "native plugin initialization failed with status {}; {}",
                    failure.status,
                    failure.cleanup.description()
                );
                match failure.cleanup {
                    crate::session::native_plugin_lifecycle::TeardownOutcome::Quiescent => {
                        if bridge_initialized {
                            drop(instance);
                            self.retain_bridge_library(
                                item.mod_id.clone(),
                                item.path.clone(),
                                item.mode,
                                library,
                            );
                            self.report_runtime_effect(
                                &item.mod_id,
                                "retained",
                                &format!("{detail}; native bridge module code remains mapped until process exit"),
                            );
                            return;
                        }
                        if let Err(error) = library.try_free() {
                            self.retain_native_module(
                                item.mod_id.clone(),
                                item.path.clone(),
                                item.mode,
                                bridge_initialized,
                                library,
                                instance,
                                "library unload",
                                crate::session::native_plugin_abi::NATIVE_STATUS_FAILED,
                            );
                            self.report_runtime_effect(
                                &item.mod_id,
                                "retained",
                                &format!("{detail}; {error}"),
                            );
                            return;
                        }
                        drop(instance);
                        self.report_runtime_effect(&item.mod_id, "error", &detail);
                    }
                    crate::session::native_plugin_lifecycle::TeardownOutcome::Retain {
                        stage,
                        status,
                    } => {
                        self.retain_native_module(
                            item.mod_id.clone(),
                            item.path.clone(),
                            item.mode,
                            bridge_initialized,
                            library,
                            instance,
                            stage,
                            status,
                        );
                        self.report_runtime_effect(&item.mod_id, "retained", &detail);
                    }
                }
                tracing::error!(target: "shroudforge::runtime", mod_id = %item.mod_id, "{detail}");
            }
        }
    }

    fn pending_plugin_is_current(&self, item: &PendingNativeDll) -> Result<bool, String> {
        if !item.mode.is_lifecycle_managed() {
            return Ok(true);
        }
        let package_root = item.package.fs().root().as_std_path().to_path_buf();
        let current_manifest = mod_loader::config::read_manifest_path(
            self.env.game_dir().as_std_path(),
            &package_root,
        )
        .map_err(|error| {
            format!("could not revalidate native plugin package before init: {error}")
        })?;
        if current_manifest.id != item.mod_id || !current_manifest.enabled {
            return Ok(false);
        }
        if !current_manifest
            .capabilities
            .contains(&Capability::RuntimeRegisterDll)
            || !current_manifest.target.supports_process(self.is_server())
        {
            return Ok(false);
        }
        let Some(declaration) = read_native_plugin_declaration(&item.package)? else {
            return Ok(false);
        };
        if !declaration.metadata.state.is_enabled()
            || native_plugin_mode(&declaration.metadata)? != item.mode
            || declaration.metadata.dll.as_deref().is_none_or(|dll| {
                normalize_plugin_path(dll) != normalize_plugin_path(&item.package_relative_path)
            })
        {
            return Ok(false);
        }
        let package_directory = package_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| "could not resolve native plugin package folder".to_owned())?;
        let selected = mod_loader::dll_manager::resolve_package_dll_path(
            self.env.game_dir().as_std_path(),
            &package_directory,
            &mod_loader::dll_manager::DllProfileFile {
                path: item.package_relative_path.clone(),
                mode: DllLoadMode::Library,
            },
        )?;
        let entry = mod_loader::dll_manager::DllLoadEntry {
            status_id: format!(
                "dll:{}:{}",
                package_directory.to_ascii_lowercase(),
                normalize_plugin_path(&item.package_relative_path)
            ),
            mod_id: Some(item.mod_id.clone()),
            package_directory,
            path: selected,
            mode: DllLoadMode::Library,
            source: "native-plugin.ini".into(),
        };
        let authorized = crate::native_dlls::authorize_deferred_plugin(
            self.env.game_dir().as_std_path(),
            self.is_server(),
            &entry,
        )?;
        Ok(authorized.declaration == declaration.metadata)
    }
    fn release_uninitialized_library(&self, mut library: NativeLibrary) -> Result<(), String> {
        if let Err(error) = library.try_free() {
            tracing::error!(target: "shroudforge::runtime", "Could not unload uninitialized native DLL: {error}");
            std::mem::forget(library);
            Err(error)
        } else {
            Ok(())
        }
    }

    fn release_or_retain_uninitialized_library(
        &self,
        mod_id: &str,
        path: &std::path::Path,
        mode: NativePluginMode,
        library: NativeLibrary,
        bridge_initialized: bool,
    ) -> Result<(), String> {
        if bridge_initialized {
            self.retain_bridge_library(mod_id.to_owned(), path.to_path_buf(), mode, library);
            Err("native bridge pointers may have escaped; module code is retained until process exit".into())
        } else {
            self.release_uninitialized_library(library)
        }
    }

    fn retain_bridge_library(
        &self,
        mod_id: String,
        path: NativePathBuf,
        mode: NativePluginMode,
        library: NativeLibrary,
    ) {
        self.native_dlls.borrow_mut().push(NativeDll {
            mod_id,
            path,
            mode,
            bridge_initialized: true,
            library,
            module: NativeModule::StoppedBridge,
        });
    }

    fn retain_native_module(
        &self,
        mod_id: String,
        path: NativePathBuf,
        mode: NativePluginMode,
        bridge_initialized: bool,
        library: NativeLibrary,
        instance: NativePluginInstance,
        stage: &'static str,
        status: i32,
    ) {
        self.native_dlls.borrow_mut().push(NativeDll {
            mod_id,
            path,
            mode,
            bridge_initialized,
            library,
            module: NativeModule::Retained {
                instance: Some(Box::new(instance)),
                stage,
                status,
            },
        });
    }

    pub(crate) fn update_native_plugins(&self, delta_seconds: f64) {
        let elapsed_microseconds = if delta_seconds.is_finite() && delta_seconds > 0.0 {
            (delta_seconds * 1_000_000.0).min(u64::MAX as f64) as u64
        } else {
            0
        };
        let mod_ids: Vec<_> = self
            .native_dlls
            .borrow()
            .iter()
            .filter(|dll| dll.is_active())
            .map(|dll| dll.mod_id.clone())
            .collect();
        let mut failed = Vec::new();
        for mod_id in mod_ids {
            let instance = {
                let mut dlls = self.native_dlls.borrow_mut();
                let Some(dll) = dlls.iter_mut().find(|dll| dll.mod_id == mod_id) else {
                    continue;
                };
                match std::mem::replace(&mut dll.module, NativeModule::Transitioning) {
                    NativeModule::Lifecycle(instance) => instance,
                    previous => {
                        dll.module = previous;
                        continue;
                    }
                }
            };
            let mut instance = Some(instance);
            let status = instance
                .as_mut()
                .expect("native plugin instance is available")
                .update(elapsed_microseconds);
            let restored = {
                let mut dlls = self.native_dlls.borrow_mut();
                if let Some(dll) = dlls.iter_mut().find(|dll| dll.mod_id == mod_id)
                    && matches!(&dll.module, NativeModule::Transitioning)
                {
                    dll.module = NativeModule::Lifecycle(
                        instance
                            .take()
                            .expect("native plugin instance is available"),
                    );
                    true
                } else {
                    false
                }
            };
            if !restored {
                // An owner vanished while its callback was running. Keep both
                // context and module mapped when ownership cannot be proven.
                if let Some(instance) = instance.take() {
                    std::mem::forget(instance);
                }
                tracing::error!(target: "shroudforge::runtime", mod_id,
                    "Retaining native plugin after update ownership changed during callback");
                continue;
            }
            if status != crate::session::native_plugin_abi::NATIVE_STATUS_OK {
                failed.push(mod_id);
            }
        }
        for mod_id in failed {
            let detail = "native plugin update failed; lifecycle teardown was requested";
            match self.stop_native_plugin(&mod_id) {
                Ok(_) => self.report_runtime_effect(&mod_id, "error", detail),
                Err(error) => {
                    self.report_runtime_effect(&mod_id, "retained", &format!("{detail}; {error}"))
                }
            }
        }
    }

    pub(crate) fn update_native_plugin_settings(
        &self,
        mod_id: &str,
        setting_values: &serde_json::Value,
    ) -> Result<(), String> {
        let pending_index = self
            .pending_native_dlls
            .borrow()
            .iter()
            .position(|pending| {
                pending.mod_id == mod_id && pending.mode == NativePluginMode::ShroudForgeV1
            });
        let active =
            self.native_dlls.borrow().iter().any(|dll| {
                dll.mod_id == mod_id && matches!(&dll.module, NativeModule::Lifecycle(_))
            });
        if pending_index.is_none() && !active {
            return Ok(());
        }
        let settings_json = serde_json::to_vec(setting_values)
            .map_err(|error| format!("could not encode native plugin settings: {error}"))?;
        if settings_json.len() > crate::session::native_plugin_abi::NATIVE_MAX_SETTINGS_JSON_BYTES {
            return Err("native plugin settings JSON exceeds 65536 bytes".into());
        }
        if let Some(index) = pending_index {
            self.pending_native_dlls.borrow_mut()[index].settings_json = settings_json.clone();
        }
        let mut dlls = self.native_dlls.borrow_mut();
        let Some(dll) = dlls.iter_mut().find(|dll| dll.mod_id == mod_id) else {
            return Ok(());
        };
        match &mut dll.module {
            NativeModule::Lifecycle(instance) => instance.update_settings_json(settings_json),
            _ => Ok(()),
        }
    }

    /// Stop one opted-in plugin. Legacy DLLs remain load-only and are retained
    /// until their session is dropped.
    pub(crate) fn stop_native_plugin(&self, mod_id: &str) -> Result<bool, String> {
        let cancelled = {
            let mut pending = self.pending_native_dlls.borrow_mut();
            cancel_pending_plugin_load(&mut pending, mod_id)
        };

        let active_index = self
            .native_dlls
            .borrow()
            .iter()
            .position(|dll| dll.mod_id == mod_id && dll.is_lifecycle_managed());
        let Some(index) = active_index else {
            if cancelled {
                self.report_runtime_effect(
                    mod_id,
                    "stopped",
                    "pending native plugin load was cancelled",
                );
            }
            return Ok(cancelled);
        };
        let (instance, mut library, bridge_initialized) = {
            let mut dlls = self.native_dlls.borrow_mut();
            let dll = &mut dlls[index];
            let previous = std::mem::replace(&mut dll.module, NativeModule::Transitioning);
            match previous {
                NativeModule::Lifecycle(instance) => {
                    let library = std::mem::replace(&mut dll.library, NativeLibrary::new(0, None));
                    (instance, library, dll.bridge_initialized)
                }
                NativeModule::Retained {
                    instance,
                    stage,
                    status,
                } => {
                    dll.module = NativeModule::Retained {
                        instance,
                        stage,
                        status,
                    };
                    let error =
                        format!("native plugin remains retained at {stage} with status {status}");
                    self.report_runtime_effect(mod_id, "retained", &error);
                    return Err(error);
                }
                NativeModule::StoppedBridge => {
                    dll.module = NativeModule::StoppedBridge;
                    self.report_runtime_effect(
                        mod_id,
                        "stopped",
                        "native plugin services are stopped; bridge module code remains mapped until process exit",
                    );
                    return Ok(true);
                }
                other => {
                    dll.module = other;
                    return Ok(cancelled);
                }
            }
        };

        let mut instance = Some(instance);
        let outcome = instance
            .as_mut()
            .expect("native plugin instance is available")
            .stop();
        let (remove, module, result) = match outcome {
            crate::session::native_plugin_lifecycle::TeardownOutcome::Quiescent
                if bridge_initialized =>
            {
                // The legacy Lua bridge registry is process-owned. Keep its
                // code mapped and forbid a second bridge initialization.
                drop(instance.take());
                (false, Some(NativeModule::StoppedBridge), Ok(()))
            }
            crate::session::native_plugin_lifecycle::TeardownOutcome::Quiescent => {
                match library.try_free() {
                    Ok(()) => {
                        drop(instance.take());
                        (true, None, Ok(()))
                    }
                    Err(error) => (
                        false,
                        Some(NativeModule::Retained {
                            instance: Some(
                                instance
                                    .take()
                                    .expect("native plugin instance is available"),
                            ),
                            stage: "library unload",
                            status: crate::session::native_plugin_abi::NATIVE_STATUS_FAILED,
                        }),
                        Err(error),
                    ),
                }
            }
            crate::session::native_plugin_lifecycle::TeardownOutcome::Retain { stage, status } => (
                false,
                Some(NativeModule::Retained {
                    instance: Some(
                        instance
                            .take()
                            .expect("native plugin instance is available"),
                    ),
                    stage,
                    status,
                }),
                Err(format!(
                    "native plugin retained at {stage} with status {status}"
                )),
            ),
        };
        let mut library = Some(library);
        let mut module = module;

        let restored = {
            let mut dlls = self.native_dlls.borrow_mut();
            if let Some(index) = dlls.iter().position(|dll| {
                dll.mod_id == mod_id && matches!(&dll.module, NativeModule::Transitioning)
            }) {
                if remove {
                    dlls.remove(index);
                } else {
                    let dll = &mut dlls[index];
                    dll.library = library.take().expect("native plugin library is present");
                    dll.module = module
                        .take()
                        .expect("retained native module state is present");
                }
                true
            } else {
                false
            }
        };
        if !restored {
            // A reentrant owner mutation makes unload authority uncertain.
            if let Some(library) = library.take() {
                std::mem::forget(library);
            }
            if let Some(module) = module.take() {
                std::mem::forget(module);
            }
            if let Some(instance) = instance.take() {
                std::mem::forget(instance);
            }
            return Err(
                "native plugin ownership changed during lifecycle teardown; module retained".into(),
            );
        }

        match result {
            Ok(()) => {
                self.report_runtime_effect(
                    mod_id,
                    "stopped",
                    if bridge_initialized {
                        "native plugin services drained; bridge module code remains mapped until process exit"
                    } else {
                        "native plugin services drained and DLL unloaded"
                    },
                );
                Ok(true)
            }
            Err(error) => {
                self.report_runtime_effect(mod_id, "retained", &error);
                Err(error)
            }
        }
    }

    pub(crate) fn restart_native_plugins(&self, mod_ids: &[String]) -> Result<(), String> {
        for mod_id in mod_ids {
            self.restart_native_plugin(mod_id)?;
        }
        Ok(())
    }

    pub(crate) fn shutdown_native_dlls(&mut self) {
        self.pending_native_dlls.get_mut().clear();
        let dlls = self.native_dlls.get_mut();
        for mut dll in dlls.drain(..).rev() {
            let module = std::mem::replace(&mut dll.module, NativeModule::Transitioning);
            match module {
                NativeModule::Lifecycle(mut instance) => match instance.stop() {
                    crate::session::native_plugin_lifecycle::TeardownOutcome::Quiescent => {
                        if dll.bridge_initialized {
                            // Bridge callbacks are process-owned even after the
                            // managed lifecycle has revoked this generation.
                            drop(instance);
                            std::mem::forget(dll);
                        } else if let Err(error) = dll.library.try_free() {
                            tracing::error!(target: "shroudforge::runtime", mod_id = %dll.mod_id,
                                    "Native plugin services drained but DLL could not be unloaded: {error}");
                            std::mem::forget((dll, instance));
                        }
                    }
                    crate::session::native_plugin_lifecycle::TeardownOutcome::Retain {
                        stage,
                        status,
                    } => {
                        tracing::error!(target: "shroudforge::runtime", mod_id = %dll.mod_id,
                                "Retaining native plugin through process exit: teardown stopped at {stage} with status {status}");
                        std::mem::forget((dll, instance));
                    }
                },
                NativeModule::Retained {
                    instance,
                    stage,
                    status,
                } => {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %dll.mod_id,
                        "Retaining native plugin through process exit: teardown stopped at {stage} with status {status}");
                    std::mem::forget((dll, instance));
                }
                NativeModule::LegacyLoadOnly => {
                    if let Err(error) = dll.library.try_free() {
                        tracing::warn!(target: "shroudforge::runtime", mod_id = %dll.mod_id,
                            "Could not unload legacy native DLL at session shutdown: {error}");
                        std::mem::forget(dll);
                    }
                }
                NativeModule::StoppedBridge => {
                    // The bridge registry owns process-lifetime callbacks into
                    // this module. Its adapter gates revoked generations.
                    std::mem::forget(dll);
                }
                NativeModule::Transitioning => {
                    tracing::error!(target: "shroudforge::runtime", mod_id = %dll.mod_id,
                        "Retaining native plugin in an incomplete lifecycle transition");
                    std::mem::forget(dll);
                }
            }
        }
    }
}

fn is_duplicate_request(
    mode: NativePluginMode,
    mod_id: &str,
    path: &std::path::Path,
    existing_mode: NativePluginMode,
    existing_mod_id: &str,
    existing_path: &std::path::Path,
) -> bool {
    if mode.is_lifecycle_managed() && existing_mode.is_lifecycle_managed() {
        mod_id == existing_mod_id
    } else {
        path == existing_path
    }
}

fn cancel_pending_plugin_load(pending: &mut Vec<PendingNativeDll>, mod_id: &str) -> bool {
    let mut cancelled = false;
    pending.retain(|item| {
        let remove = item.mod_id == mod_id && item.mode == NativePluginMode::ShroudForgeV1;
        cancelled |= remove;
        !remove
    });
    cancelled
}

pub(super) struct NativeDll {
    pub(super) mod_id: String,
    pub(super) path: NativePathBuf,
    mode: NativePluginMode,
    bridge_initialized: bool,
    library: NativeLibrary,
    module: NativeModule,
}

impl NativeDll {
    fn mode(&self) -> NativePluginMode {
        self.mode
    }

    fn is_lifecycle_managed(&self) -> bool {
        matches!(
            &self.module,
            NativeModule::Lifecycle(_)
                | NativeModule::Retained { .. }
                | NativeModule::StoppedBridge
        )
    }

    fn is_active(&self) -> bool {
        matches!(&self.module, NativeModule::Lifecycle(_))
    }

    fn is_retained(&self) -> bool {
        matches!(&self.module, NativeModule::Retained { .. })
    }
}

enum NativeModule {
    LegacyLoadOnly,
    Lifecycle(Box<NativePluginInstance>),
    Retained {
        instance: Option<Box<NativePluginInstance>>,
        stage: &'static str,
        status: i32,
    },
    StoppedBridge,
    Transitioning,
}

pub(super) struct PendingNativeDll {
    pub(super) mod_id: String,
    pub(super) path: NativePathBuf,
    package_relative_path: String,
    mode: NativePluginMode,
    package: Mod,
    package_generation: u64,
    settings_json: Vec<u8>,
    queued_at: std::time::Instant,
    warned_stall: bool,
    receiver: Receiver<Result<LoadedNativeLibrary, NativeLibraryLoadError>>,
}

struct NativeLibrary {
    handle: isize,
    temporary_directory: Option<NativePathBuf>,
}

impl NativeLibrary {
    fn new(handle: isize, temporary_directory: Option<NativePathBuf>) -> Self {
        Self {
            handle,
            temporary_directory,
        }
    }

    fn entrypoint(&self, mode: NativePluginMode) -> Result<Option<NativePluginEntry>, String> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
            super::native_plugin_abi::entrypoint_for_mode(mode, || {
                let symbol = unsafe {
                    GetProcAddress(
                        self.handle as windows_sys::Win32::Foundation::HMODULE,
                        b"ShroudForgePluginEntry\0".as_ptr(),
                    )
                };
                symbol.map(|symbol| unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, NativePluginEntry>(
                        symbol,
                    )
                })
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (self, mode);
            Err("native plugin lifecycle lookup is supported only on Windows".into())
        }
    }

    fn try_free(&mut self) -> Result<(), String> {
        if self.handle == 0 {
            self.remove_temporary_directory();
            return Ok(());
        }
        #[cfg(windows)]
        {
            let result = unsafe {
                windows_sys::Win32::Foundation::FreeLibrary(
                    self.handle as windows_sys::Win32::Foundation::HMODULE,
                )
            };
            if result == 0 {
                return Err(format!(
                    "FreeLibrary failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            self.handle = 0;
            self.remove_temporary_directory();
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err("native DLL unloading is supported only on Windows".into())
        }
    }

    fn remove_temporary_directory(&mut self) {
        if let Some(directory) = self.temporary_directory.take() {
            if let Err(error) = std::fs::remove_dir_all(&directory) {
                tracing::warn!(target: "shroudforge::runtime", %error, path = %directory.display(),
                    "Could not remove extracted native DLL files");
            }
        }
    }
}

impl Drop for NativeLibrary {
    fn drop(&mut self) {
        if self.handle != 0 {
            #[cfg(windows)]
            {
                let result = unsafe {
                    windows_sys::Win32::Foundation::FreeLibrary(
                        self.handle as windows_sys::Win32::Foundation::HMODULE,
                    )
                };
                if result == 0 {
                    if let Some(directory) = self.temporary_directory.take() {
                        // Keep extracted code available while Windows still maps it.
                        std::mem::forget(directory);
                    }
                    return;
                }
                self.handle = 0;
            }
            #[cfg(not(windows))]
            {
                return;
            }
        }
        self.remove_temporary_directory();
    }
}

#[cfg(windows)]
fn load_native_library(
    mod_id: &str,
    path: &std::path::Path,
    temporary_directory: Option<NativePathBuf>,
) -> Result<NativeLibrary, String> {
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
        Ok(NativeLibrary::new(handle as isize, temporary_directory))
    }
}

#[cfg(not(windows))]
fn load_native_library(
    mod_id: &str,
    _path: &std::path::Path,
    _temporary_directory: Option<NativePathBuf>,
) -> Result<NativeLibrary, String> {
    Err(format!(
        "native DLL loading for mod '{mod_id}' is supported only on Windows"
    ))
}

fn load_native_library_with_bridge(
    mod_id: &str,
    path: &std::path::Path,
    temporary_directory: Option<NativePathBuf>,
    package_directory: &std::path::Path,
) -> Result<LoadedNativeLibrary, NativeLibraryLoadError> {
    let library = load_native_library(mod_id, path, temporary_directory)
        .map_err(NativeLibraryLoadError::Load)?;
    match crate::native_bridge::initialize(library.handle, Some(mod_id), package_directory) {
        Ok(bridge_initialized) => Ok(LoadedNativeLibrary {
            library,
            bridge_initialized,
        }),
        Err(detail) => Err(NativeLibraryLoadError::Bridge { library, detail }),
    }
}

fn isolated_load_native_library(
    mod_id: String,
    path: NativePathBuf,
    temporary_directory: Option<NativePathBuf>,
    package_directory: NativePathBuf,
) -> Result<LoadedNativeLibrary, NativeLibraryLoadError> {
    let (sender, receiver) = mpsc::channel();
    let worker_mod_id = mod_id.clone();
    let worker_path = path.clone();
    let worker_package_directory = package_directory.clone();
    let worker_tempdir = temporary_directory.clone();
    spawn_retained_native_worker(
        format!("sf-native-{}", mod_id.chars().take(24).collect::<String>()),
        crate::native_dlls::pin_native_runtime_module,
        move || {
            let result = load_native_library_with_bridge(
                &worker_mod_id,
                &worker_path,
                worker_tempdir,
                &worker_package_directory,
            );
            match sender.send(result) {
                Ok(()) => {}
                Err(error) => match error.0 {
                    Ok(loaded) if loaded.bridge_initialized => {
                        // The process-owned bridge registry stores callbacks
                        // into this module; keep the code mapped until exit.
                        std::mem::forget(loaded.library);
                    }
                    Err(NativeLibraryLoadError::Bridge { library, .. }) => {
                        // The bridge initializer received a stable host pointer.
                        std::mem::forget(library);
                    }
                    _ => {}
                },
            }
        },
    )
    .map_err(NativeLibraryLoadError::Load)?;
    receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|error| {
            NativeLibraryLoadError::Load(match error {
                mpsc::RecvTimeoutError::Timeout => {
                    format!("isolated native DLL load for mod '{mod_id}' timed out")
                }
                mpsc::RecvTimeoutError::Disconnected => {
                    format!("isolated native DLL load for mod '{mod_id}' exited without a result")
                }
            })
        })?
}

#[cfg(test)]
mod tests {
    use super::{
        LoadedNativeLibrary, NativeLibrary, NativePluginMode, PendingNativeDll,
        SelectedNativePluginState, SelectedNativePluginsThreadOwner, StagedNativePluginBatch,
        cancel_pending_plugin_load, is_duplicate_request, spawn_retained_native_worker,
    };

    struct OwnerDropProbe(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl Drop for OwnerDropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    struct RetainedOwnerProbe {
        bridge_initialized: bool,
        // Separate counters model the loaded module, plugin instance, boxed
        // host table, and opaque context held by a staged plugin.
        parts: [OwnerDropProbe; 4],
    }

    thread_local! {
        static TEST_SELECTED_NATIVE_PLUGINS: SelectedNativePluginsThreadOwner<RetainedOwnerProbe> =
            const { SelectedNativePluginsThreadOwner::new() };
    }

    struct DropTrackedServices(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl Drop for DropTrackedServices {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl crate::session::native_plugin_abi::NativePluginServiceLifetime for DropTrackedServices {
        unsafe fn query_service(
            &self,
            _package_id: &str,
            _package_generation: u64,
            _service_name: &str,
            _requested_version: u32,
            _out_capacity: u32,
            _out_table: *mut std::ffi::c_void,
        ) -> i32 {
            crate::session::native_plugin_abi::NATIVE_STATUS_UNAVAILABLE
        }

        fn revoke(&self, _package_id: &str, _package_generation: u64) -> i32 {
            crate::session::native_plugin_abi::NATIVE_STATUS_OK
        }

        fn drain(&self, _package_id: &str, _package_generation: u64, _timeout_ms: u32) -> i32 {
            crate::session::native_plugin_abi::NATIVE_STATUS_OK
        }
    }

    struct RestoredCallbackGate {
        entered: mpsc::Sender<()>,
        release: std::sync::Mutex<mpsc::Receiver<()>>,
    }

    unsafe extern "C" fn wait_for_restored_callback_release(
        plugin_context: *mut std::ffi::c_void,
    ) -> i32 {
        let gate = unsafe { &*plugin_context.cast::<RestoredCallbackGate>() };
        gate.entered.send(()).unwrap();
        gate.release.lock().unwrap().recv().unwrap();
        crate::session::native_plugin_abi::NATIVE_STATUS_OK
    }

    #[test]
    fn owner_thread_exit_retains_prepared_batch_while_restored_callback_is_running() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let owner_drops: [Arc<AtomicUsize>; 9] =
            std::array::from_fn(|_| Arc::new(AtomicUsize::new(0)));
        let thread_owner_drops = owner_drops.clone();

        let (owner_ready, owner_is_ready) = mpsc::channel();
        let (owner_timeout, owner_exits) = mpsc::channel();
        let owner = std::thread::spawn(move || {
            let services = Arc::new(DropTrackedServices(thread_owner_drops[0].clone()));
            let plugin_probe = |bridge_initialized, index: usize| RetainedOwnerProbe {
                bridge_initialized,
                parts: std::array::from_fn(|part| {
                    OwnerDropProbe(thread_owner_drops[1 + index * 4 + part].clone())
                }),
            };
            let plugins = vec![plugin_probe(false, 0), plugin_probe(true, 1)];
            assert!(!plugins[0].bridge_initialized && plugins[1].bridge_initialized);
            let batch = StagedNativePluginBatch::<RetainedOwnerProbe> {
                root: PathBuf::from("prepared-owner-test"),
                is_server: false,
                services: services.clone(),
                plugins,
            };
            drop(services);
            TEST_SELECTED_NATIVE_PLUGINS.with(|state| {
                *state.try_borrow_mut().unwrap() = SelectedNativePluginState::Prepared(batch);
            });
            owner_ready.send(()).unwrap();
            owner_exits.recv().unwrap();
            // Returning models the bootstrap owner leaving after the post-restore
            // deadline while the original-entry callback is still in progress.
        });
        owner_is_ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();

        let (callback_entered, callback_waiting) = mpsc::channel();
        let (callback_release, callback_released) = mpsc::channel();
        let callback_gate = Box::new(RestoredCallbackGate {
            entered: callback_entered,
            release: std::sync::Mutex::new(callback_released),
        });
        let callback_context = (&*callback_gate as *const RestoredCallbackGate) as usize;
        let callback = std::thread::spawn(move || unsafe {
            wait_for_restored_callback_release(callback_context as *mut std::ffi::c_void)
        });
        callback_waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();

        owner_timeout.send(()).unwrap();
        owner.join().unwrap();
        assert_eq!(
            owner_drops
                .each_ref()
                .map(|count| count.load(Ordering::SeqCst)),
            [0; 9],
            "the prepared owner and its host services must outlive an executing callback"
        );

        callback_release.send(()).unwrap();
        assert_eq!(
            callback.join().unwrap(),
            crate::session::native_plugin_abi::NATIVE_STATUS_OK
        );
        assert_eq!(
            owner_drops
                .each_ref()
                .map(|count| count.load(Ordering::SeqCst)),
            [0; 9]
        );
    }

    #[test]
    fn native_worker_requires_residency_before_detached_execution() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let retained = Arc::new(AtomicBool::new(false));
        let observed = retained.clone();
        let (release, wait) = mpsc::channel::<()>();
        let (finished, completion) = mpsc::channel();
        let worker = spawn_retained_native_worker(
            "native-residency-test".into(),
            || {
                retained.store(true, Ordering::Release);
                Ok(())
            },
            move || {
                assert!(observed.load(Ordering::Acquire));
                let _ = wait.recv();
                finished.send(()).unwrap();
            },
        )
        .unwrap();
        // Session cancellation can drop its receiver/handle without releasing
        // the process-resident code that the pending worker will return into.
        drop(worker);
        drop(release);
        completion
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();

        let called = Arc::new(AtomicBool::new(false));
        let observed = called.clone();
        let error = spawn_retained_native_worker(
            "native-residency-refused".into(),
            || Err("residency refused".into()),
            move || {
                observed.store(true, Ordering::Release);
            },
        )
        .unwrap_err();
        assert_eq!(error, "residency refused");
        assert!(!called.load(Ordering::Acquire));
    }
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::Arc,
        sync::mpsc,
        time::Instant,
    };

    #[test]
    fn duplicate_opted_in_start_is_rejected_by_package_identity() {
        assert!(is_duplicate_request(
            NativePluginMode::ShroudForgeV1,
            "example",
            Path::new("one.dll"),
            NativePluginMode::ShroudForgeV1,
            "example",
            Path::new("other.dll"),
        ));
        assert!(is_duplicate_request(
            NativePluginMode::ShroudForgeV1,
            "example",
            Path::new("one.dll"),
            NativePluginMode::LegacyLoadOnly,
            "example",
            Path::new("one.dll"),
        ));
    }

    #[test]
    fn disable_cancels_completed_pending_load_before_owner_thread_initialization() {
        let root = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("native-plugin-cancel-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let (sender, receiver) = mpsc::channel();
        assert!(
            sender
                .send(Ok(LoadedNativeLibrary {
                    library: NativeLibrary::new(0, Some(root.clone())),
                    bridge_initialized: false,
                }))
                .is_ok()
        );
        let package_root = root.join("mods").join("example");
        fs::create_dir_all(&package_root).unwrap();
        fs::write(
            package_root.join("mod.json"),
            r#"{"id":"example","name":"Example","version":"1.0.0","capabilities":["runtime-register-dll"]}"#,
        )
        .unwrap();
        fs::write(
            package_root.join("extended.mod.json"),
            r#"{"schemaVersion":1,"enabled":true,"targets":["client"],"settings":{}}"#,
        )
        .unwrap();
        let environment =
            mod_loader::ModEnvironment::load(camino::Utf8Path::from_path(&root).unwrap()).unwrap();
        let package = environment.mod_registry().get("example").unwrap().clone();
        let (other_sender, other_receiver) = mpsc::channel();
        let mut pending = vec![
            PendingNativeDll {
                mod_id: "example".into(),
                path: PathBuf::from("example.dll"),
                package_relative_path: "example.dll".into(),
                mode: NativePluginMode::ShroudForgeV1,
                package,
                package_generation: 1,
                settings_json: b"{}".to_vec(),
                queued_at: Instant::now(),
                warned_stall: false,
                receiver,
            },
            PendingNativeDll {
                mod_id: "example".into(),
                path: PathBuf::from("legacy.dll"),
                package_relative_path: "legacy.dll".into(),
                mode: NativePluginMode::LegacyLoadOnly,
                package: environment.mod_registry().get("example").unwrap().clone(),
                package_generation: 2,
                settings_json: b"{}".to_vec(),
                queued_at: Instant::now(),
                warned_stall: false,
                receiver: other_receiver,
            },
        ];

        assert!(cancel_pending_plugin_load(&mut pending, "example"));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].mode, NativePluginMode::LegacyLoadOnly);
        assert!(
            !root.exists(),
            "dropping the queued library cleans its extraction directory"
        );
        drop(other_sender);
        drop(pending);
        let _ = fs::remove_dir_all(root);
    }
}
