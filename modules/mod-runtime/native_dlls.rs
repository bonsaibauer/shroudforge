//! Process-owned execution of the DLL Manager's load plan and Lua registrations.
//! Native hooks outlive Lua sessions. Handles are deliberately retained until exit.

use mod_loader::dll_manager::{
    DllLoadEntry, DllLoadMode, PluginIniDeclaration, PluginLoadDisposition,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};

const STARTUP_LOAD_TIMEOUT: Duration = Duration::from_secs(15);
static APPLIED_PLAN: OnceLock<AppliedPlan> = OnceLock::new();

#[derive(Clone)]
pub(crate) struct DeferredNativePlugin {
    pub(crate) entry: DllLoadEntry,
    pub(crate) declaration: PluginIniDeclaration,
}

struct AppliedPlan {
    root_key: String,
    is_server: bool,
    identity: serde_json::Value,
    entries: Vec<DeferredNativePlugin>,
    rejected_entries: Vec<RejectedNativePlugin>,
}

#[derive(Clone)]
struct RejectedNativePlugin {
    entry: DllLoadEntry,
    declaration: Option<PluginIniDeclaration>,
    detail: String,
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
            pin_native_runtime_module()?;
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
    let declaration = selected_plugin_declaration(root, &entry)?;
    if let Some(plan) = APPLIED_PLAN.get() {
        if plan.root_key != root_key(root)? {
            return Err("DLL Manager startup plan belongs to a different game directory".into());
        }
        ensure_prepared_entry_unchanged(plan, &entry, declaration.as_ref())?;
    }
    if let Some(declaration) = declaration {
        match mod_loader::dll_manager::classify_plugin_load(&declaration, true)? {
            PluginLoadDisposition::Ordinary => {}
            PluginLoadDisposition::ShroudForgeV1 => {
                if entry.mode != DllLoadMode::Library {
                    return Err("Lifecycle-managed plugins require library loading mode".into());
                }
                if !is_authorized_deferred_entry(root, &entry)? {
                    return Err(
                        "Lifecycle-managed plugin is not present in the current approved DLL Manager plan. Enable and approve its package selection, then restart the game".into(),
                    );
                }
                tracing::debug!(target: "shroudforge::dll_manager", dll = %entry.path.display(),
                    "Deferring lifecycle plugin initialization to its runtime owner");
                return Ok(());
            }
        }
    }
    if crate::startup_services::ordinary_already_loaded(root, &entry)? {
        return Ok(());
    }
    queue_process_library(root, entry)
}

fn selected_plugin_declaration(
    root: &Path,
    entry: &DllLoadEntry,
) -> Result<Option<PluginIniDeclaration>, String> {
    mod_loader::dll_manager::selected_plugin_ini_declaration(
        root,
        &entry.package_directory,
        &entry.path,
    )
}

fn queue_process_library(root: &Path, entry: DllLoadEntry) -> Result<(), String> {
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

fn same_entry(left: &DllLoadEntry, right: &DllLoadEntry) -> bool {
    left.status_id == right.status_id
        && left.mod_id == right.mod_id
        && left
            .package_directory
            .eq_ignore_ascii_case(&right.package_directory)
        && left.path == right.path
        && left.mode == right.mode
        && left.source == right.source
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

fn same_plan_path(left: &DllLoadEntry, right: &DllLoadEntry) -> bool {
    left.status_id == right.status_id
        && left.mod_id == right.mod_id
        && left
            .package_directory
            .eq_ignore_ascii_case(&right.package_directory)
        && left.path == right.path
}

fn ensure_prepared_entry_unchanged(
    plan: &AppliedPlan,
    entry: &DllLoadEntry,
    declaration: Option<&PluginIniDeclaration>,
) -> Result<(), String> {
    if let Some(prepared) = plan
        .entries
        .iter()
        .find(|prepared| same_plan_path(&prepared.entry, entry))
    {
        if prepared.entry.mode != entry.mode || declaration != Some(&prepared.declaration) {
            return Err(
                "Lifecycle plugin selection or declaration changed after startup preparation; restart the game".into(),
            );
        }
    }
    if let Some(rejected) = plan
        .rejected_entries
        .iter()
        .find(|rejected| same_plan_path(&rejected.entry, entry))
    {
        return Err(format!(
            "Native plugin was rejected during startup and cannot fall back to ordinary loading: {}",
            rejected.detail
        ));
    }
    Ok(())
}

fn rejected_plan_matches(
    current: &[RejectedNativePlugin],
    prepared: &[RejectedNativePlugin],
) -> bool {
    current.len() == prepared.len()
        && current.iter().all(|candidate| {
            prepared.iter().any(|original| {
                same_entry(&candidate.entry, &original.entry)
                    && candidate.declaration == original.declaration
                    && candidate.detail == original.detail
            })
        })
}

fn deferred_plan_authorizes(
    plan: &AppliedPlan,
    root_key: &str,
    is_server: bool,
    identity: &serde_json::Value,
    entry: &DllLoadEntry,
) -> bool {
    plan.root_key == root_key
        && plan.is_server == is_server
        && &plan.identity == identity
        && plan
            .entries
            .iter()
            .any(|candidate| same_selected_path(&candidate.entry, entry))
}
fn classify_plan_entries(
    root: &Path,
    entries: Vec<DllLoadEntry>,
) -> (
    Vec<DllLoadEntry>,
    Vec<DeferredNativePlugin>,
    Vec<RejectedNativePlugin>,
    Vec<mod_loader::dll_manager::DllPlanIssue>,
) {
    let (mut ordinary, mut deferred, mut rejected, mut issues) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for entry in entries {
        let declaration = match selected_plugin_declaration(root, &entry) {
            Ok(Some(declaration)) => declaration,
            Ok(None) => {
                ordinary.push(entry);
                continue;
            }
            Err(detail) => {
                issues.push(mod_loader::dll_manager::DllPlanIssue {
                    status_id: entry.status_id.clone(),
                    detail: detail.clone(),
                });
                rejected.push(RejectedNativePlugin {
                    entry,
                    declaration: None,
                    detail,
                });
                continue;
            }
        };
        match mod_loader::dll_manager::classify_plugin_load(&declaration, true) {
            Ok(PluginLoadDisposition::ShroudForgeV1) => {
                if entry.mode != DllLoadMode::Library {
                    let detail =
                        "Lifecycle-managed plugins require library loading mode".to_owned();
                    issues.push(mod_loader::dll_manager::DllPlanIssue {
                        status_id: entry.status_id.clone(),
                        detail: detail.clone(),
                    });
                    rejected.push(RejectedNativePlugin {
                        entry,
                        declaration: Some(declaration),
                        detail,
                    });
                } else {
                    deferred.push(DeferredNativePlugin { entry, declaration });
                }
            }
            Ok(PluginLoadDisposition::Ordinary) => ordinary.push(entry),
            Err(detail) => {
                issues.push(mod_loader::dll_manager::DllPlanIssue {
                    status_id: entry.status_id.clone(),
                    detail: detail.clone(),
                });
                rejected.push(RejectedNativePlugin {
                    entry,
                    declaration: Some(declaration),
                    detail,
                });
            }
        }
    }
    (ordinary, deferred, rejected, issues)
}

/// Inspect the approved selection without loading a DLL or starting a worker.
pub(crate) fn selected_startup_plugins(
    root: &Path,
    is_server: bool,
) -> (Vec<DllLoadEntry>, Vec<DeferredNativePlugin>, bool) {
    let plan = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    let (ordinary, plugins, rejected, issues) = classify_plan_entries(root, plan.entries);
    (
        ordinary,
        plugins,
        !plan.issues.is_empty() || !rejected.is_empty() || !issues.is_empty(),
    )
}

pub(crate) fn deferred_native_plugins(
    root: &Path,
    is_server: bool,
) -> Result<Vec<DeferredNativePlugin>, String> {
    let plan = APPLIED_PLAN
        .get()
        .ok_or_else(|| "DLL Manager startup plan has not been applied".to_owned())?;
    if plan.root_key != root_key(root)? || plan.is_server != is_server {
        return Err(
            "DLL Manager startup plan belongs to a different process or game directory".into(),
        );
    }

    let current = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    if mod_loader::dll_manager::load_plan_identity(&current) != plan.identity {
        return Err("DLL Manager selection changed after startup preparation; restart the game before loading native plugins".into());
    }
    let (_, current_deferred, current_rejected, _) = classify_plan_entries(root, current.entries);
    if current_deferred.len() != plan.entries.len()
        || current_deferred
            .iter()
            .zip(&plan.entries)
            .any(|(current, original)| {
                !same_entry(&current.entry, &original.entry)
                    || current.declaration != original.declaration
            })
    {
        return Err("A lifecycle plugin declaration changed after startup preparation; restart the game before loading native plugins".into());
    }
    if !rejected_plan_matches(&current_rejected, &plan.rejected_entries) {
        return Err("A rejected native plugin declaration changed after startup preparation; restart the game before loading native plugins".into());
    }
    Ok(plan.entries.clone())
}

pub(crate) fn authorize_deferred_plugin(
    root: &Path,
    is_server: bool,
    entry: &DllLoadEntry,
) -> Result<DeferredNativePlugin, String> {
    deferred_native_plugins(root, is_server)?
        .into_iter()
        .find(|candidate| same_selected_path(&candidate.entry, entry))
        .ok_or_else(|| {
            "Lifecycle-managed plugin is not in the current approved DLL Manager plan".into()
        })
}
fn is_authorized_deferred_entry(root: &Path, entry: &DllLoadEntry) -> Result<bool, String> {
    let Some(plan) = APPLIED_PLAN.get() else {
        return Ok(false);
    };
    let key = root_key(root)?;
    if key != plan.root_key {
        return Ok(false);
    }
    let _ = deferred_native_plugins(root, plan.is_server)?;
    Ok(deferred_plan_authorizes(
        plan,
        &key,
        plan.is_server,
        &plan.identity,
        entry,
    ))
}
/// Called while the bootstrap holds the game entrypoint, outside the loader lock.
/// File errors remain local to their DLL. A worker that never finishes must not
/// release the game into an unknown, possibly locked initialization state.
pub(crate) fn prepare_selected_startup(root: &Path, is_server: bool) -> Result<(), String> {
    if APPLIED_PLAN.get().is_some() {
        return Err("Native plan was applied before selected admission".into());
    }
    let plan = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    let identity = mod_loader::dll_manager::load_plan_identity(&plan);
    let (_, deferred, rejected_entries, issues) = classify_plan_entries(root, plan.entries);
    if !issues.is_empty() || !plan.issues.is_empty() || !rejected_entries.is_empty() {
        return Err("Selected native plan is unresolved".into());
    }
    APPLIED_PLAN
        .set(AppliedPlan {
            root_key: root_key(root)?,
            is_server,
            identity,
            entries: deferred,
            rejected_entries,
        })
        .map_err(|_| "Native startup plan was already published".into())
}

pub fn prepare_startup(root: &Path, is_server: bool) -> Result<(), String> {
    apply_plan(root, is_server, true)
}

pub(crate) fn apply_plan(root: &Path, is_server: bool, wait: bool) -> Result<(), String> {
    if APPLIED_PLAN.get().is_some() {
        return Ok(());
    }
    let enabled = manager_enabled(root)?;
    let plan = mod_loader::dll_manager::resolve_load_plan(root, is_server);
    let root_key = root_key(root)?;
    let identity = mod_loader::dll_manager::load_plan_identity(&plan);
    let mut issues = plan.issues;
    let (ordinary, deferred, rejected_entries, classified_issues) =
        classify_plan_entries(root, plan.entries);
    issues.extend(classified_issues);
    if APPLIED_PLAN
        .set(AppliedPlan {
            root_key,
            is_server,
            identity,
            entries: deferred,
            rejected_entries,
        })
        .is_err()
    {
        return Ok(());
    }
    if !enabled {
        tracing::info!(target: "shroudforge::dll_manager", "DLL Manager is disabled. Native DLL loading is skipped");
        return Ok(());
    }
    if ordinary.is_empty() && issues.is_empty() {
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
    for entry in ordinary {
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
mod managed_plugin_plan_tests {
    use super::*;

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

    #[test]
    fn managed_plan_entry_is_deferred_and_never_queued_as_ordinary() {
        let fixture = Fixture::new();
        fixture.package(
            "managed",
            "[Plugin]\nEnabled=true\nDll=bin/plugin.dll\nLifecycle=shroudforge-v1\nServices=shroudforge.overlay\n",
            &["bin/plugin.dll", "bin/other.dll"],
        );
        let plugin = fixture.entry("managed", "bin/plugin.dll", DllLoadMode::Library);
        let other = fixture.entry("managed", "bin/other.dll", DllLoadMode::Library);

        let (ordinary, deferred, _rejected, issues) =
            classify_plan_entries(&fixture.0, vec![plugin.clone(), other.clone()]);

        assert!(issues.is_empty());
        assert_eq!(ordinary.len(), 1);
        assert!(same_entry(&ordinary[0], &other));
        assert_eq!(deferred.len(), 1);
        assert!(same_entry(&deferred[0].entry, &plugin));
        assert!(
            deferred[0]
                .declaration
                .declares_service("shroudforge.overlay")
        );
    }

    #[test]
    fn disabled_declared_lifecycle_entry_remains_managed() {
        let fixture = Fixture::new();
        fixture.package(
            "disabled",
            "[Plugin]\nEnabled=false\nDll=plugin.dll\nLifecycle=shroudforge-v1\n",
            &["plugin.dll"],
        );
        let plugin = fixture.entry("disabled", "plugin.dll", DllLoadMode::Library);

        let (ordinary, deferred, rejected, issues) =
            classify_plan_entries(&fixture.0, vec![plugin]);

        assert!(ordinary.is_empty());
        assert!(rejected.is_empty());
        assert!(issues.is_empty());
        assert_eq!(deferred.len(), 1);
        assert!(!deferred[0].declaration.state.is_enabled());
    }

    #[test]
    fn malformed_declared_plugin_is_rejected_without_ordinary_fallback() {
        let fixture = Fixture::new();
        fixture.package(
            "malformed",
            "[Plugin]\nEnabled=true\nDll=plugin.dll\nDll=other.dll\nLifecycle=shroudforge-v1\n",
            &["plugin.dll", "other.dll"],
        );
        let plugin = fixture.entry("malformed", "plugin.dll", DllLoadMode::Library);

        let (ordinary, deferred, rejected, issues) =
            classify_plan_entries(&fixture.0, vec![plugin]);

        assert!(ordinary.is_empty());
        assert!(deferred.is_empty());
        assert_eq!(rejected.len(), 1);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].detail.contains("invalid [Plugin] declaration"));
    }

    #[test]
    fn unsupported_lifecycle_creates_plan_error_without_ordinary_fallback() {
        let fixture = Fixture::new();
        fixture.package(
            "unsupported",
            "[Plugin]\nEnabled=true\nDll=plugin.dll\nLifecycle=other-v9\n",
            &["plugin.dll"],
        );
        let plugin = fixture.entry("unsupported", "plugin.dll", DllLoadMode::Library);

        let (ordinary, deferred, rejected, issues) =
            classify_plan_entries(&fixture.0, vec![plugin]);

        assert!(ordinary.is_empty());
        assert!(deferred.is_empty());
        assert_eq!(rejected.len(), 1);
        assert_eq!(issues.len(), 1);
        assert!(
            issues[0]
                .detail
                .contains("unsupported native plugin lifecycle")
        );
        assert!(rejected[0].declaration.is_some());
    }

    #[test]
    fn rejected_lifecycle_cannot_downgrade_to_ordinary_after_preparation() {
        let fixture = Fixture::new();
        let package = fixture.package(
            "unsupported",
            "[Plugin]\nEnabled=true\nDll=plugin.dll\nLifecycle=other-v9\n",
            &["plugin.dll"],
        );
        let plugin = fixture.entry("unsupported", "plugin.dll", DllLoadMode::Library);
        let (_, _, prepared_rejected, _) = classify_plan_entries(&fixture.0, vec![plugin.clone()]);
        assert_eq!(prepared_rejected.len(), 1);
        let prepared_plan = AppliedPlan {
            root_key: root_key(&fixture.0).unwrap(),
            is_server: false,
            identity: serde_json::Value::Null,
            entries: Vec::new(),
            rejected_entries: prepared_rejected.clone(),
        };

        std::fs::write(
            package.join("native-plugin.ini"),
            "[Plugin]\nEnabled=true\nDll=plugin.dll\n",
        )
        .unwrap();
        let (ordinary, _, current_rejected, _) =
            classify_plan_entries(&fixture.0, vec![plugin.clone()]);

        assert_eq!(ordinary.len(), 1);
        assert!(current_rejected.is_empty());
        assert!(!rejected_plan_matches(
            &current_rejected,
            &prepared_rejected
        ));
        let current_declaration = selected_plugin_declaration(&fixture.0, &plugin).unwrap();
        assert!(
            ensure_prepared_entry_unchanged(&prepared_plan, &plugin, current_declaration.as_ref(),)
                .is_err()
        );
    }

    #[test]
    fn deferred_authority_requires_exact_root_process_plan_and_selected_path() {
        let fixture = Fixture::new();
        fixture.package(
            "managed",
            "[Plugin]\nEnabled=true\nDll=plugin.dll\nLifecycle=shroudforge-v1\n",
            &["plugin.dll", "other.dll"],
        );
        let selected = fixture.entry("managed", "plugin.dll", DllLoadMode::Library);
        let denied = fixture.entry("managed", "other.dll", DllLoadMode::Library);
        let identity = serde_json::json!([{
            "statusId": selected.status_id.clone(),
            "path": selected.path.to_string_lossy(),
            "mode": selected.mode
        }]);
        let plan = AppliedPlan {
            root_key: root_key(&fixture.0).unwrap(),
            is_server: false,
            identity: identity.clone(),
            entries: vec![DeferredNativePlugin {
                entry: selected.clone(),
                declaration: mod_loader::dll_manager::read_plugin_ini_declaration(
                    &fixture.0.join("mods/managed"),
                ),
            }],
            rejected_entries: Vec::new(),
        };

        assert!(deferred_plan_authorizes(
            &plan,
            &root_key(&fixture.0).unwrap(),
            false,
            &identity,
            &selected,
        ));
        assert!(!deferred_plan_authorizes(
            &plan,
            &root_key(&fixture.0).unwrap(),
            true,
            &identity,
            &selected,
        ));
        assert!(!deferred_plan_authorizes(
            &plan,
            "another-game-root",
            false,
            &identity,
            &selected,
        ));
        assert!(!deferred_plan_authorizes(
            &plan,
            &root_key(&fixture.0).unwrap(),
            false,
            &serde_json::Value::Null,
            &selected,
        ));
        assert!(!deferred_plan_authorizes(
            &plan,
            &root_key(&fixture.0).unwrap(),
            false,
            &identity,
            &denied,
        ));
    }
}
