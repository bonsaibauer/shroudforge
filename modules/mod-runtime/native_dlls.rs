//! Process-owned execution of the DLL Manager's load plan and Lua registrations.
//! Native hooks outlive Lua sessions. Handles are deliberately retained until exit.

use mod_loader::dll_manager::{DllLoadEntry, DllLoadMode};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};

const STARTUP_LOAD_TIMEOUT: Duration = Duration::from_secs(15);
static APPLIED_PLAN: OnceLock<serde_json::Value> = OnceLock::new();
static EXECUTOR: OnceLock<Result<Executor, String>> = OnceLock::new();

pub(crate) struct NativeDllStatus {
    pub id: String,
    pub state: &'static str,
    pub detail: String,
    pub source: String,
}

struct Record {
    mode: DllLoadMode,
    ids: HashSet<String>,
    state: &'static str,
    detail: String,
    source: String,
    // Retaining the Windows reference prevents Lua reloads from unloading hooks.
    _handle: Option<isize>,
}

#[derive(Default)]
struct State {
    records: HashMap<String, Record>,
    issues: BTreeMap<String, String>,
}

struct Executor {
    sender: mpsc::Sender<(String, PathBuf, DllLoadEntry)>,
    shared: Arc<(Mutex<State>, Condvar)>,
}

fn executor() -> Result<&'static Executor, String> {
    EXECUTOR
        .get_or_init(|| {
            #[cfg(windows)]
            pin_executor_module()?;
            let (sender, receiver) = mpsc::channel::<(String, PathBuf, DllLoadEntry)>();
            let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
            let worker_state = shared.clone();
            std::thread::Builder::new()
                .name("sf-native-dlls".into())
                .spawn(move || {
                    while let Ok((key, root, entry)) = receiver.recv() {
                        tracing::debug!(target: "shroudforge::dll_manager", status_id = %entry.status_id,
                            dll = %entry.path.display(), source = %entry.source, mode = ?entry.mode,
                            "Loading native DLL on the ordered process worker");
                        let (handle, result) = load_library(&root, &entry);
                        // The optional bridge export runs right after the load, still in load order.
                        let result = result.and_then(|()| match handle {
                            Some(handle) => crate::native_bridge::initialize(
                                handle,
                                entry.mod_id.as_deref(),
                                &shroudforge_loader_config::paths::mods_dir(&root)
                                    .join(&entry.package_directory),
                            ),
                            None => Ok(false),
                        });
                        let (state_name, detail) = match result {
                            Ok(bridged) => {
                                tracing::info!(target: "shroudforge::dll_manager", status_id = %entry.status_id,
                                    dll = %entry.path.display(), bridged, "Native DLL loaded. Mod functionality is not independently verified");
                                ("loaded", if bridged {
                                    "Native DLL loaded for this process and connected to its mod's Lua code. Functionality is not independently verified"
                                } else {
                                    "Native DLL loaded for this process. Functionality is not independently verified"
                                }.to_owned())
                            }
                            Err(error) => {
                                tracing::error!(target: "shroudforge::dll_manager", status_id = %entry.status_id,
                                    dll = %entry.path.display(), %error, "Native DLL could not be initialized");
                                ("error", error)
                            }
                        };
                        let (state, completed) = &*worker_state;
                        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
                        if let Some(record) = state.records.get_mut(&key) {
                            record.state = state_name;
                            record.detail = detail;
                            record._handle = handle;
                        }
                        completed.notify_all();
                    }
                })
                .map_err(|error| format!("could not start the native DLL worker: {error}"))?;
            Ok(Executor { sender, shared })
        })
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(windows)]
fn pin_executor_module() -> Result<(), String> {
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleHandleExW,
    };

    // The bootstrap can release its runtime-library reference while this worker
    // and installed hooks still run. Keep their owning code mapped until exit.
    let mut module = std::ptr::null_mut();
    let pinned = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            executor as *const () as *const u16,
            &mut module,
        )
    };
    if pinned == 0 {
        return Err(format!(
            "could not retain the native DLL executor's runtime module: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

pub(crate) fn manager_enabled(root: &Path) -> Result<bool, String> {
    let config = shroudforge_loader_config::config::read_loader(root)?;
    Ok(config
        .pointer("/modules/dllManager/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true))
}

pub(crate) fn queue(root: &Path, mut entry: DllLoadEntry) -> Result<(), String> {
    if !manager_enabled(root)? {
        return Err(
            "DLL Manager is disabled. Enable it and restart the game to load native DLLs".into(),
        );
    }
    if mod_loader::dll_manager::read_selections(root)
        .iter()
        .any(|selection| {
            selection
                .package_directory
                .eq_ignore_ascii_case(&entry.package_directory)
                && (!selection.enabled || !selection.approved)
        })
    {
        return Err("Native DLL loading for this mod folder is disabled or not confirmed in DLL Manager. Update the selection and restart the game".into());
    }
    if mod_loader::dll_manager::is_excluded_dll_name(root, &entry.path.to_string_lossy()) {
        return Err("This DLL is excluded by the DLL Manager filename filter".into());
    }
    entry.path = entry
        .path
        .canonicalize()
        .map_err(|error| format!("could not resolve native DLL path: {error}"))?;
    if !entry.path.is_file() {
        return Err("The selected native DLL is not a regular file".into());
    }
    let key = path_key(&entry.path);
    let executor = executor()?;
    let mut state = executor
        .shared
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut ids = HashSet::from([entry.status_id.clone()]);
    if let Some(mod_id) = &entry.mod_id {
        ids.insert(mod_id.clone());
    }
    if let Some(record) = state.records.get_mut(&key) {
        if record.mode != entry.mode && entry.mode != DllLoadMode::Library {
            return Err("This DLL was already scheduled with another initialization mode. Restart the game after changing its selection".into());
        }
        record.ids.extend(ids);
        tracing::trace!(target: "shroudforge::dll_manager", dll = %entry.path.display(),
            "Reusing the process-owned native DLL registration");
        return Ok(());
    }
    state.records.insert(
        key.clone(),
        Record {
            mode: entry.mode,
            ids,
            state: "loading",
            detail: "Native DLL queued in the declared load order".into(),
            source: entry.source.clone(),
            _handle: None,
        },
    );
    if executor
        .sender
        .send((key.clone(), root.to_path_buf(), entry))
        .is_err()
    {
        state.records.remove(&key);
        return Err("The native DLL worker is no longer available".into());
    }
    Ok(())
}

/// Called while the bootstrap holds the game entrypoint, outside the loader lock.
/// File errors remain local to their DLL. A worker that never finishes must not
/// release the game into an unknown, possibly locked initialization state.
pub fn prepare_startup(root: &Path, is_server: bool) -> Result<(), String> {
    apply_plan(root, is_server, true)
}

pub(crate) fn apply_plan(root: &Path, is_server: bool, wait: bool) -> Result<(), String> {
    if APPLIED_PLAN.get().is_some() {
        return Ok(());
    }
    let enabled = manager_enabled(root)?;
    let plan = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    if APPLIED_PLAN
        .set(mod_loader::dll_manager::load_plan_identity(&plan))
        .is_err()
    {
        return Ok(());
    }
    if !enabled {
        tracing::info!(target: "shroudforge::dll_manager", "DLL Manager is disabled. Native DLL loading is skipped");
        return Ok(());
    }
    if plan.entries.is_empty() && plan.issues.is_empty() {
        return Ok(());
    }
    let executor = executor()?;
    for issue in plan.issues {
        tracing::warn!(target: "shroudforge::dll_manager", status_id = %issue.status_id,
            detail = %issue.detail, "Native DLL selection needs attention");
        executor
            .shared
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .issues
            .insert(issue.status_id, issue.detail);
    }
    for entry in plan.entries {
        let status_id = entry.status_id.clone();
        if let Err(error) = queue(root, entry) {
            tracing::error!(target: "shroudforge::dll_manager", %status_id, %error,
                "Native DLL could not be scheduled");
            executor
                .shared
                .0
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .issues
                .insert(status_id, error);
        }
    }
    if wait {
        let deadline = Instant::now() + STARTUP_LOAD_TIMEOUT;
        let (state, completed) = &*executor.shared;
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        while state
            .records
            .values()
            .any(|record| record.state == "loading")
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let pending = state
                    .records
                    .iter()
                    .find(|(_, record)| record.state == "loading")
                    .map(|(path, _)| path.as_str())
                    .unwrap_or("unknown");
                return Err(format!(
                    "Native DLL '{pending}' did not finish initialization within 15 seconds. Disable it in DLL Manager and restart the game"
                ));
            }
            state = completed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|error| error.into_inner())
                .0;
        }
    }
    Ok(())
}

pub(crate) fn applied_plan() -> serde_json::Value {
    APPLIED_PLAN
        .get()
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

pub(crate) fn statuses() -> Vec<NativeDllStatus> {
    let Some(Ok(executor)) = EXECUTOR.get() else {
        return Vec::new();
    };
    let state = executor
        .shared
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut statuses = BTreeMap::<String, NativeDllStatus>::new();
    let priority = |state| match state {
        "error" => 3,
        "loading" => 2,
        _ => 1,
    };
    for record in state.records.values() {
        for id in &record.ids {
            if statuses
                .get(id)
                .is_none_or(|previous| priority(previous.state) < priority(record.state))
            {
                statuses.insert(
                    id.clone(),
                    NativeDllStatus {
                        id: id.clone(),
                        state: record.state,
                        detail: record.detail.clone(),
                        source: record.source.clone(),
                    },
                );
            }
        }
    }
    for (id, detail) in &state.issues {
        statuses.insert(
            id.clone(),
            NativeDllStatus {
                id: id.clone(),
                state: "error",
                detail: detail.clone(),
                source: "plan".into(),
            },
        );
    }
    statuses.into_values().collect()
}

fn path_key(path: &Path) -> String {
    let key = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

#[cfg(windows)]
fn load_library(root: &Path, entry: &DllLoadEntry) -> (Option<isize>, Result<(), String>) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{
            GetModuleHandleW, GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
        },
    };
    type SetDinputProvider = unsafe extern "system" fn(HMODULE) -> i32;
    let provider_setter: Option<SetDinputProvider> = if entry.mode == DllLoadMode::Dinput8 {
        // The game proxy, System32 and native mods may all use this basename.
        // Resolve the installation's host explicitly instead of picking one.
        let host_path = match root.join("dinput8.dll").canonicalize() {
            Ok(path) => path,
            Err(error) => {
                return (
                    None,
                    Err(format!(
                        "ShroudForge's dinput8 chain host could not be resolved in the game folder: {error}. Install the matching ShroudForge dinput8.dll and restart"
                    )),
                );
            }
        };
        let host_name: Vec<u16> = host_path.as_os_str().encode_wide().chain(Some(0)).collect();
        tracing::trace!(target: "shroudforge::dll_manager", host = %host_path.display(),
            "Looking up the installation's DirectInput chain host");
        let host = unsafe { GetModuleHandleW(host_name.as_ptr()) };
        let setter = if host.is_null() {
            None
        } else {
            unsafe { GetProcAddress(host, c"ShroudforgeSetDinput8Provider".as_ptr().cast()) }
        };
        let Some(setter) = setter else {
            return (None, Err("This DLL needs ShroudForge's dinput8 chain host. Install the matching ShroudForge dinput8.dll in the game folder and restart".into()));
        };
        Some(unsafe { std::mem::transmute(setter) })
    } else {
        None
    };
    let wide_path: Vec<u16> = entry
        .path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let handle = unsafe {
        LoadLibraryExW(
            wide_path.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        )
    };
    if handle.is_null() {
        return (
            None,
            Err(format!(
                "Windows could not load the native DLL: {}",
                std::io::Error::last_os_error()
            )),
        );
    }
    if let Some(setter) = provider_setter {
        if unsafe { GetProcAddress(handle, c"DirectInput8Create".as_ptr().cast()) }.is_none() {
            if entry.source != "chain" {
                return (Some(handle as isize), Err("The selected DLL does not export DirectInput8Create. Select ordinary DLL loading instead of the DirectInput start method".into()));
            }
            tracing::debug!(target: "shroudforge::dll_manager", dll = %entry.path.display(),
                "Chain DLL loaded without a DirectInput provider export");
            return (Some(handle as isize), Ok(()));
        }
        let accepted = unsafe { setter(handle) } != 0;
        tracing::debug!(target: "shroudforge::dll_manager", dll = %entry.path.display(), accepted,
            "DirectInput chain provider considered. The first provider keeps forwarded calls");
    }
    (Some(handle as isize), Ok(()))
}

#[cfg(not(windows))]
fn load_library(_root: &Path, _entry: &DllLoadEntry) -> (Option<isize>, Result<(), String>) {
    (
        None,
        Err("Native DLL loading is available only on Windows".into()),
    )
}
