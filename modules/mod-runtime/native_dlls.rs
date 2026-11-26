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
static APPLIED_PLAN: OnceLock<AppliedPlan> = OnceLock::new();

#[derive(Clone)]
pub(crate) struct RegisteredNativePlugin {
    pub(crate) entry: DllLoadEntry,
    pub(crate) registration: crate::native_bridge::RegisteredLifecycle,
}

struct AppliedPlan {
    root_key: String,
    is_server: bool,
    identity: serde_json::Value,
    entries: Vec<DllLoadEntry>,
}

static EXECUTOR: OnceLock<Result<Executor, String>> = OnceLock::new();

pub(crate) struct NativeDllStatus {
    pub id: String,
    pub state: &'static str,
    pub detail: String,
    pub source: String,
}

struct Record {
    mode: DllLoadMode,
    mod_id: Option<String>,
    session_owned: bool,
    ids: HashSet<String>,
    state: &'static str,
    detail: String,
    source: String,
    // The result owns the Windows reference for process life, including errors
    // after initialization began. Session receivers only borrow that handle.
    result: Option<ProcessLibraryResult>,
    waiters: Vec<mpsc::Sender<ProcessLibraryResult>>,
}

impl Record {
    fn complete(&mut self, result: ProcessLibraryResult) {
        self.result = Some(result.clone());
        for waiter in self.waiters.drain(..) {
            let _ = waiter.send(result.clone());
        }
    }
}

#[derive(Clone)]
pub(crate) struct ProcessLibraryResult {
    pub(crate) handle: Option<isize>,
    pub(crate) initialization: Result<bool, String>,
    pub(crate) lifecycle_factory: Option<crate::native_bridge::RegisteredLifecycle>,
}

struct LoadRequest {
    key: String,
    root: PathBuf,
    entry: DllLoadEntry,
}

#[derive(Default)]
struct State {
    records: HashMap<String, Record>,
    issues: BTreeMap<String, String>,
}

struct Executor {
    sender: mpsc::Sender<LoadRequest>,
    shared: Arc<(Mutex<State>, Condvar)>,
}

fn executor() -> Result<&'static Executor, String> {
    EXECUTOR
        .get_or_init(|| {
            pin_native_runtime_module()?;
            let (sender, receiver) = mpsc::channel::<LoadRequest>();
            let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
            let worker_state = shared.clone();
            std::thread::Builder::new()
                .name("sf-native-dlls".into())
                .spawn(move || {
                    while let Ok(LoadRequest { key, root, entry }) = receiver.recv() {
                        tracing::debug!(target: "shroudforge::dll_manager", status_id = %entry.status_id,
                            dll = %entry.path.display(), source = %entry.source, mode = ?entry.mode,
                            "Loading native DLL on the ordered process worker");
                        let (handle, result) = load_library(&root, &entry);
                        // The optional bridge export runs right after the load, still in load order.
                        let result = result.and_then(|()| match handle {
                            Some(handle) => crate::native_bridge::initialize(
                                handle,
                                entry.mod_id.as_deref(),
                                &shroudforge_loader_config::paths::mods_dir(&root).join(&entry.package_directory),
                            ),
                            None => Ok(crate::native_bridge::NativeRegistration::default()),
                        });
                        let lifecycle_factory = result.as_ref().ok().and_then(|registration| registration.lifecycle_factory);
                        let result = result.map(|registration| registration.connected);
                        let (state_name, detail) = match &result {
                            Ok(bridged) => {
                                tracing::info!(target: "shroudforge::dll_manager", status_id = %entry.status_id,
                                    dll = %entry.path.display(), bridged, "Native DLL loaded. Mod functionality is not independently verified");
                                ("loaded", if *bridged {
                                    "Native DLL loaded for this process and connected to its mod's native API. Functionality is not independently verified"
                                } else {
                                    "Native DLL loaded for this process. Functionality is not independently verified"
                                }.to_owned())
                            }
                            Err(error) => {
                                tracing::error!(target: "shroudforge::dll_manager", status_id = %entry.status_id,
                                    dll = %entry.path.display(), %error, "Native DLL could not be initialized");
                                ("error", error.clone())
                            }
                        };
                        let (state, completed) = &*worker_state;
                        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
                        if let Some(record) = state.records.get_mut(&key) {
                            record.state = state_name;
                            record.detail = detail;
                            let result = ProcessLibraryResult { handle, initialization: result, lifecycle_factory };
                            record.session_owned |= result.lifecycle_factory.is_some();
                            record.complete(result);
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
pub(crate) fn pin_native_runtime_module() -> Result<(), String> {
    use windows_sys::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleHandleExW,
    };

    // The bootstrap can release its runtime-library reference while this worker
    // and installed hooks still run. Keep their owning code mapped until exit.
    let mut module = std::ptr::null_mut();
    let pinned = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            pin_native_runtime_module as *const () as *const u16,
            &mut module,
        )
    };
    if pinned == 0 {
        return Err(format!(
            "could not retain the native host runtime module: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn pin_native_runtime_module() -> Result<(), String> {
    Ok(())
}

pub(crate) fn manager_enabled(root: &Path) -> Result<bool, String> {
    let config = shroudforge_loader_config::config::read_loader(root)?;
    Ok(config
        .pointer("/modules/dllManager/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true))
}

pub(crate) fn queue(root: &Path, entry: DllLoadEntry) -> Result<(), String> {
    if let Some(plan) = APPLIED_PLAN.get() {
        if plan.root_key != root_key(root)? {
            return Err("DLL Manager startup plan belongs to a different game directory".into());
        }
    }
    queue_process_library(root, entry)
}

fn queue_process_library(root: &Path, entry: DllLoadEntry) -> Result<(), String> {
    queue_package_library(root, entry, false).map(drop)
}

/// All package and selected requests share the same authorization checks and
/// ordered executor. A cached load never grants missing package permissions.
pub(crate) fn queue_package_library(
    root: &Path,
    mut entry: DllLoadEntry,
    session_owned: bool,
) -> Result<mpsc::Receiver<ProcessLibraryResult>, String> {
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
    queue_package_library_on(executor()?, root, entry, session_owned)
}

fn queue_package_library_on(
    executor: &Executor,
    root: &Path,
    entry: DllLoadEntry,
    session_owned: bool,
) -> Result<mpsc::Receiver<ProcessLibraryResult>, String> {
    let key = path_key(&entry.path);
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
        if record.mod_id != entry.mod_id {
            return Err("This DLL is already owned by another mod. Restart the game after changing its package identity".into());
        }
        record.ids.extend(ids);
        record.session_owned |= session_owned;
        tracing::trace!(target: "shroudforge::dll_manager", dll = %entry.path.display(),
            "Reusing the process-owned native DLL registration");
        let (sender, receiver) = mpsc::channel();
        if let Some(result) = &record.result {
            let _ = sender.send(result.clone());
        } else {
            record.waiters.push(sender);
        }
        return Ok(receiver);
    }
    let (sender, receiver) = mpsc::channel();
    state.records.insert(
        key.clone(),
        Record {
            mode: entry.mode,
            mod_id: entry.mod_id.clone(),
            session_owned,
            ids,
            state: "loading",
            detail: "Native DLL queued in the declared load order".into(),
            source: entry.source.clone(),
            result: None,
            waiters: vec![sender],
        },
    );
    if executor
        .sender
        .send(LoadRequest {
            key: key.clone(),
            root: root.to_path_buf(),
            entry,
        })
        .is_err()
    {
        state.records.remove(&key);
        return Err("The native DLL worker is no longer available".into());
    }
    Ok(receiver)
}

fn root_key(root: &Path) -> Result<String, String> {
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("could not resolve game directory for DLL plan: {error}"))?;
    let key = canonical.to_string_lossy().into_owned();
    Ok(if cfg!(windows) {
        key.to_ascii_lowercase()
    } else {
        key
    })
}

fn same_selected_path(left: &DllLoadEntry, right: &DllLoadEntry) -> bool {
    left.status_id == right.status_id
        && left.mod_id == right.mod_id
        && left
            .package_directory
            .eq_ignore_ascii_case(&right.package_directory)
        && left.path == right.path
        && left.mode == right.mode
}

/// Accepted SDK registrations from the one process-owned DLL plan.
/// No DLL is loaded or factory called while reading this metadata.
pub(crate) fn registered_native_plugins(
    root: &Path,
    is_server: bool,
) -> Result<Vec<RegisteredNativePlugin>, String> {
    let Some(plan) = APPLIED_PLAN.get() else {
        return Ok(Vec::new());
    };
    if plan.root_key != root_key(root)? || plan.is_server != is_server {
        return Err(
            "DLL Manager startup plan belongs to a different process or game directory".into(),
        );
    }
    let current = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    if mod_loader::dll_manager::load_plan_identity(&current) != plan.identity {
        return Err(
            "DLL Manager selection changed after startup preparation; restart the game".into(),
        );
    }
    let Some(Ok(executor)) = EXECUTOR.get() else {
        return Ok(Vec::new());
    };
    let state = executor
        .shared
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    collect_registered_plugins(&plan.entries, &state)
}

fn collect_registered_plugins(
    entries: &[DllLoadEntry],
    state: &State,
) -> Result<Vec<RegisteredNativePlugin>, String> {
    let mut packages = HashMap::new();
    let mut plugins = Vec::new();
    for entry in entries {
        let Some(registration) = state
            .records
            .get(&path_key(&entry.path))
            .and_then(|record| record.result.as_ref())
            .filter(|result| result.initialization.is_ok())
            .and_then(|result| result.lifecycle_factory)
        else {
            continue;
        };
        if entry.mode != DllLoadMode::Library {
            return Err("SDK lifecycle callbacks require library loading mode".into());
        }
        let id = entry
            .mod_id
            .as_ref()
            .ok_or("SDK lifecycle callbacks require a manifest-backed package")?;
        if let Some(previous) = packages.insert(id, path_key(&entry.path)) {
            if previous == path_key(&entry.path) {
                continue;
            }
            return Err(format!(
                "Package '{id}' registered lifecycle callbacks from more than one DLL"
            ));
        }
        plugins.push(RegisteredNativePlugin {
            entry: entry.clone(),
            registration,
        });
    }
    Ok(plugins)
}

pub(crate) fn package_has_registered_lifecycle(mod_id: &str) -> bool {
    let Some(Ok(executor)) = EXECUTOR.get() else {
        return false;
    };
    executor
        .shared
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .records
        .values()
        .any(|record| {
            record.mod_id.as_deref() == Some(mod_id)
                && record
                    .result
                    .as_ref()
                    .is_some_and(|result| result.lifecycle_factory.is_some())
        })
}

pub(crate) fn authorize_registered_plugin(
    root: &Path,
    is_server: bool,
    entry: &DllLoadEntry,
) -> Result<RegisteredNativePlugin, String> {
    registered_native_plugins(root, is_server)?
        .into_iter()
        .find(|candidate| same_selected_path(&candidate.entry, entry))
        .ok_or_else(|| {
            "Native callback registration is not in the current approved DLL plan".into()
        })
}
/// Called while the bootstrap holds the game entrypoint, outside the loader lock.
/// File errors remain local to their DLL. A worker that never finishes must not
/// release the game into an unknown, possibly locked initialization state.
pub(crate) fn prepare_selected_startup(
    root: &Path,
    is_server: bool,
    expected: &serde_json::Value,
) -> Result<(), String> {
    if APPLIED_PLAN.get().is_some() {
        return Err("Native plan was applied before selected admission".into());
    }
    let plan = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    if mod_loader::dll_manager::load_plan_identity(&plan) != *expected {
        return Err("DLL Manager plan changed after startup admission".into());
    }
    apply_resolved_plan(root, is_server, true, plan)
}

pub fn prepare_startup(root: &Path, is_server: bool) -> Result<(), String> {
    apply_plan(root, is_server, true)
}

pub(crate) fn apply_plan(root: &Path, is_server: bool, wait: bool) -> Result<(), String> {
    if APPLIED_PLAN.get().is_some() {
        if wait {
            if let Some(Ok(executor)) = EXECUTOR.get() {
                wait_for_libraries(executor)?;
            }
        }
        return Ok(());
    }
    let plan = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    apply_resolved_plan(root, is_server, wait, plan)
}

// One execution path for ordinary startup and the retained admission snapshot.
fn apply_resolved_plan(
    root: &Path,
    is_server: bool,
    wait: bool,
    plan: mod_loader::dll_manager::DllLoadPlan,
) -> Result<(), String> {
    let enabled = manager_enabled(root)?;
    let root_key = root_key(root)?;
    let identity = mod_loader::dll_manager::load_plan_identity(&plan);
    let issues = plan.issues;
    let entries = plan.entries;
    if APPLIED_PLAN
        .set(AppliedPlan {
            root_key,
            is_server,
            identity,
            entries: entries.clone(),
        })
        .is_err()
    {
        return Ok(());
    }
    if !enabled {
        tracing::info!(target: "shroudforge::dll_manager", "DLL Manager is disabled. Native DLL loading is skipped");
        return Ok(());
    }
    if entries.is_empty() && issues.is_empty() {
        return Ok(());
    }
    let executor = executor()?;
    for issue in issues {
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
    for entry in entries {
        let status_id = entry.status_id.clone();
        let queued = queue(root, entry);
        if let Err(error) = queued {
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
        wait_for_libraries(executor)?;
    }
    Ok(())
}

fn wait_for_libraries(executor: &Executor) -> Result<(), String> {
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
    Ok(())
}

pub(crate) fn applied_plan() -> serde_json::Value {
    APPLIED_PLAN
        .get()
        .map(|plan| plan.identity.clone())
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
            // Session activation reports its own callbacks and failures. A
            // successful OS load must not overwrite that mod's lifecycle state.
            if record.session_owned && record.mod_id.as_ref() == Some(id) {
                continue;
            }
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
pub(crate) fn load_library(
    root: &Path,
    entry: &DllLoadEntry,
) -> (Option<isize>, Result<(), String>) {
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
pub(crate) fn load_library(
    _root: &Path,
    _entry: &DllLoadEntry,
) -> (Option<isize>, Result<(), String>) {
    (
        None,
        Err("Native DLL loading is available only on Windows".into()),
    )
}
#[cfg(test)]
mod process_plan_tests {
    use super::*;

    unsafe extern "C" fn test_lifecycle_factory(
        _abi: u32,
        _size: u32,
        _output: *mut crate::session::SFNativePluginV1,
    ) -> i32 {
        2
    }

    fn controlled_executor() -> (Executor, mpsc::Receiver<LoadRequest>) {
        let (sender, receiver) = mpsc::channel();
        (
            Executor {
                sender,
                shared: Arc::new((Mutex::new(State::default()), Condvar::new())),
            },
            receiver,
        )
    }

    #[test]
    fn package_and_selected_requests_share_manager_approval_and_exclusion_checks() {
        for scenario in ["disabled", "unapproved", "excluded"] {
            let fixture = Fixture::new();
            fixture.package("denied", "", &["plugin.dll"]);
            let mut config = shroudforge_loader_config::config::read_loader_defaults().unwrap();
            match scenario {
                "disabled" => config["modules"]["dllManager"]["enabled"] = false.into(),
                "unapproved" => {
                    config["modules"]["dllManager"]["profiles"] = serde_json::json!([{
                        "id":"denied", "packageDirectory":"denied", "name":"Denied", "target":"client",
                        "enabled":true, "approved":false, "files":[{"path":"plugin.dll", "mode":"library"}]
                    }])
                }
                _ => {
                    config["modules"]["dllManager"]["excludedDllNames"] =
                        serde_json::json!(["plugin.dll"])
                }
            }
            let path = shroudforge_loader_config::paths::loader_config(&fixture.0);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, serde_json::to_vec(&config).unwrap()).unwrap();
            let entry = fixture.entry("denied", "plugin.dll", DllLoadMode::Library);
            let error = queue_package_library(&fixture.0, entry, true)
                .err()
                .unwrap();
            assert!(
                error.contains(match scenario {
                    "disabled" => "DLL Manager is disabled",
                    "unapproved" => "not confirmed",
                    _ => "excluded",
                }),
                "{error}"
            );
        }
    }

    #[test]
    fn sdk_metadata_is_read_once_per_canonical_dll_and_rejects_ambiguous_package_owners() {
        let fixture = Fixture::new();
        fixture.package("sdk", "", &["one.dll", "two.dll"]);
        let one = fixture.entry("sdk", "one.dll", DllLoadMode::Library);
        let two = fixture.entry("sdk", "two.dll", DllLoadMode::Library);
        // Build the same process records through a live controlled queue.
        let (executor, work) = controlled_executor();
        for entry in [&one, &two] {
            let _ = queue_package_library_on(&executor, &fixture.0, entry.clone(), false).unwrap();
            let request = work.recv().unwrap();
            executor
                .shared
                .0
                .lock()
                .unwrap()
                .records
                .get_mut(&request.key)
                .unwrap()
                .complete(ProcessLibraryResult {
                    handle: Some(42),
                    initialization: Ok(true),
                    lifecycle_factory: Some(crate::native_bridge::RegisteredLifecycle {
                        factory: test_lifecycle_factory,
                        mode: crate::session::NativePluginMode::ShroudForgeV2,
                    }),
                });
        }
        let state = executor.shared.0.lock().unwrap();
        let registrations =
            collect_registered_plugins(&[one.clone(), one.clone()], &state).unwrap();
        assert_eq!(registrations.len(), 1);
        assert!(matches!(
            registrations[0].registration.mode,
            crate::session::NativePluginMode::ShroudForgeV2
        ));
        assert!(
            collect_registered_plugins(&[one.clone(), two], &state)
                .err()
                .unwrap()
                .contains("more than one DLL")
        );
        let mut wrong_mode = one;
        wrong_mode.mode = DllLoadMode::Dinput8;
        assert!(
            collect_registered_plugins(&[wrong_mode], &state)
                .err()
                .unwrap()
                .contains("library loading mode")
        );
    }

    #[test]
    fn ordinary_and_managed_requests_share_pending_and_completed_loads() {
        let fixture = Fixture::new();
        fixture.package("shared", "", &["plugin.dll"]);
        let entry = fixture.entry("shared", "plugin.dll", DllLoadMode::Library);
        let (executor, work) = controlled_executor();
        let ordinary =
            queue_package_library_on(&executor, &fixture.0, entry.clone(), false).unwrap();
        let cancelled =
            queue_package_library_on(&executor, &fixture.0, entry.clone(), true).unwrap();
        let managed = queue_package_library_on(&executor, &fixture.0, entry.clone(), true).unwrap();
        drop(cancelled);
        let request = work.try_recv().unwrap();
        assert!(
            work.try_recv().is_err(),
            "one OS load and bridge initialization for all requests"
        );
        assert!(ordinary.try_recv().is_err() && managed.try_recv().is_err());
        {
            let mut state = executor.shared.0.lock().unwrap();
            let record = state.records.get_mut(&request.key).unwrap();
            assert!(
                record.session_owned,
                "OS status must not replace lifecycle status"
            );
            record.complete(ProcessLibraryResult {
                handle: Some(42),
                initialization: Ok(true),
                lifecycle_factory: Some(crate::native_bridge::RegisteredLifecycle {
                    factory: test_lifecycle_factory,
                    mode: crate::session::NativePluginMode::ShroudForgeV1,
                }),
            });
        }
        for result in [ordinary.recv().unwrap(), managed.recv().unwrap()] {
            assert_eq!(result.handle, Some(42));
            assert_eq!(result.initialization, Ok(true));
            assert!(std::ptr::fn_addr_eq(
                result.lifecycle_factory.unwrap().factory,
                test_lifecycle_factory as crate::native_bridge::NativeLifecycleFactory
            ));
        }
        let replay = queue_package_library_on(&executor, &fixture.0, entry, true).unwrap();
        let replay = replay.recv().unwrap();
        assert_eq!(replay.handle, Some(42));
        assert!(std::ptr::fn_addr_eq(
            replay.lifecycle_factory.unwrap().factory,
            test_lifecycle_factory as crate::native_bridge::NativeLifecycleFactory
        ));
        assert!(
            work.try_recv().is_err(),
            "reload must reuse the process result"
        );
        assert_eq!(
            executor.shared.0.lock().unwrap().records[&request.key]
                .result
                .as_ref()
                .unwrap()
                .handle,
            Some(42)
        );
    }

    #[test]
    fn failed_bridge_is_retained_and_never_initialized_again() {
        let fixture = Fixture::new();
        fixture.package("failed", "", &["plugin.dll"]);
        let mut entry = fixture.entry("failed", "plugin.dll", DllLoadMode::Library);
        let (executor, work) = controlled_executor();
        let pending = queue_package_library_on(&executor, &fixture.0, entry.clone(), true).unwrap();
        let request = work.try_recv().unwrap();
        executor
            .shared
            .0
            .lock()
            .unwrap()
            .records
            .get_mut(&request.key)
            .unwrap()
            .complete(ProcessLibraryResult {
                handle: Some(43),
                initialization: Err("bridge refused".into()),
                lifecycle_factory: None,
            });
        assert_eq!(
            pending.recv().unwrap().initialization,
            Err("bridge refused".into())
        );
        let replay = queue_package_library_on(&executor, &fixture.0, entry.clone(), true)
            .unwrap()
            .recv()
            .unwrap();
        assert_eq!(replay.handle, Some(43));
        assert_eq!(replay.initialization, Err("bridge refused".into()));
        entry.mod_id = Some("different.owner".into());
        assert!(queue_package_library_on(&executor, &fixture.0, entry, true).is_err());
        assert!(work.try_recv().is_err());
    }

    #[test]
    fn mixed_requests_preserve_order_and_cannot_change_load_mode() {
        let fixture = Fixture::new();
        fixture.package("ordered", "", &["one.dll", "two.dll"]);
        let one = fixture.entry("ordered", "one.dll", DllLoadMode::Library);
        let two = fixture.entry("ordered", "two.dll", DllLoadMode::Library);
        let (executor, work) = controlled_executor();
        let _one = queue_package_library_on(&executor, &fixture.0, one.clone(), false).unwrap();
        let _two = queue_package_library_on(&executor, &fixture.0, two.clone(), true).unwrap();
        assert_eq!(work.try_recv().unwrap().entry.path, one.path);
        assert_eq!(work.try_recv().unwrap().entry.path, two.path);
        let mut changed_mode = one;
        changed_mode.mode = DllLoadMode::Dinput8;
        assert!(queue_package_library_on(&executor, &fixture.0, changed_mode, true).is_err());
        assert!(work.try_recv().is_err());
    }

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("sf-managed-native-{}-{nonce}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn package(&self, name: &str, ini: &str, files: &[&str]) -> PathBuf {
            let package = self.0.join("mods").join(name);
            std::fs::create_dir_all(&package).unwrap();
            std::fs::write(package.join("native-plugin.ini"), ini).unwrap();
            for file in files {
                let path = package.join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"fixture").unwrap();
            }
            package
        }

        fn entry(&self, directory: &str, file: &str, mode: DllLoadMode) -> DllLoadEntry {
            DllLoadEntry {
                status_id: format!(
                    "dll:{}:{}",
                    directory.to_ascii_lowercase(),
                    file.to_ascii_lowercase()
                ),
                mod_id: Some("fixture.plugin".into()),
                package_directory: directory.into(),
                path: self
                    .0
                    .join("mods")
                    .join(directory)
                    .join(file)
                    .canonicalize()
                    .unwrap(),
                mode,
                source: "profile".into(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
