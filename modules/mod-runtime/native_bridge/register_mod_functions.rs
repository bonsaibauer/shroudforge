use super::*;
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    path::Path,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy)]
struct Registered {
    function: NativeFunction,
    context: usize,
    host: usize,
}

/// Functions by owning mod id, then by name. A mod sees only its own entry.
static FUNCTIONS: Mutex<BTreeMap<String, BTreeMap<String, Registered>>> =
    Mutex::new(BTreeMap::new());

/// One host per initialized DLL, kept until exit because the DLL may log
/// from its own threads at any time.
#[repr(C)]
pub(crate) struct HostRecord {
    // First field: the DLL receives this address and hands it back.
    host: ModHost,
    mod_id: String,
    accepting: AtomicBool,
    _strings: [CString; 2],
}

impl HostRecord {
    pub(crate) fn host(&'static self) -> *const ModHost {
        &self.host
    }

    pub(super) fn mod_id(&self) -> &str {
        &self.mod_id
    }
}

fn functions() -> MutexGuard<'static, BTreeMap<String, BTreeMap<String, Registered>>> {
    FUNCTIONS.lock().unwrap_or_else(|error| error.into_inner())
}

/// `host` must be null or a pointer ShroudForge handed to a DLL: only hosts
/// created by open_host are handed out, so the cast is sound for those.
pub(super) unsafe fn record<'a>(host: *const ModHost) -> Option<&'a HostRecord> {
    (!host.is_null()).then(|| unsafe { &*host.cast::<HostRecord>() })
}

/// Calls the optional init export of a freshly loaded DLL on the native DLL
/// worker. Returns whether the DLL takes part in the bridge.
#[cfg(windows)]
pub(crate) fn initialize(
    handle: isize,
    mod_id: Option<&str>,
    mod_directory: &Path,
) -> Result<bool, String> {
    use windows_sys::Win32::System::LibraryLoader::GetProcAddress;

    let Some(export) = (unsafe { GetProcAddress(handle as _, INIT_EXPORT.as_ptr().cast()) })
    else {
        return Ok(false);
    };
    let Some(mod_id) = mod_id else {
        return Err("The DLL exports ShroudforgeModNativeInit, but its folder has no mod.json. Native functions belong to a mod".into());
    };
    let init: InitFunction = unsafe { std::mem::transmute(export) };
    let record = open_host(mod_id, mod_directory)?;
    tracing::debug!(target: "shroudforge::dll_manager", mod_id,
        "Initializing the native bridge of a mod DLL");
    let accepted = unsafe { init(record.host()) } != 0;
    close_registration(record, accepted);
    if !accepted {
        return Err("ShroudforgeModNativeInit reported a failure. Its native functions are unavailable".into());
    }
    tracing::info!(target: "shroudforge::dll_manager", mod_id,
        functions = function_names(mod_id).len(), "Native bridge initialized");
    Ok(true)
}

#[cfg(not(windows))]
pub(crate) fn initialize(
    _handle: isize,
    _mod_id: Option<&str>,
    _mod_directory: &Path,
) -> Result<bool, String> {
    Ok(false)
}

pub(crate) fn open_host(mod_id: &str, mod_directory: &Path) -> Result<&'static HostRecord, String> {
    let id = CString::new(mod_id).map_err(|_| "The mod id contains a NUL character")?;
    let directory = CString::new(mod_directory.to_string_lossy().into_owned())
        .map_err(|_| "The mod folder path contains a NUL character")?;
    Ok(Box::leak(Box::new(HostRecord {
        host: ModHost {
            abi_version: ABI_VERSION,
            size: size_of::<ModHost>() as u32,
            mod_id: id.as_ptr(),
            mod_directory: directory.as_ptr(),
            log: host_log,
            register_function: host_register_function,
            read_local_camera: super::read_local_camera::host_read_local_camera,
        },
        mod_id: mod_id.to_owned(),
        accepting: AtomicBool::new(true),
        _strings: [id, directory],
    })))
}

/// Ends the registration window. A failed initialization withdraws what
/// this DLL registered, so Lua never calls into a DLL that gave up.
pub(crate) fn close_registration(record: &'static HostRecord, keep: bool) {
    let mut functions = functions();
    record.accepting.store(false, Ordering::Release);
    if !keep {
        let host = record.host() as usize;
        if let Some(owned) = functions.get_mut(&record.mod_id) {
            owned.retain(|_, registered| registered.host != host);
        }
    }
}

pub(crate) fn function_names(mod_id: &str) -> Vec<String> {
    functions()
        .get(mod_id)
        .map(|owned| owned.keys().cloned().collect())
        .unwrap_or_default()
}

pub(super) fn find(mod_id: &str, name: &str) -> Option<(NativeFunction, usize)> {
    functions()
        .get(mod_id)
        .and_then(|owned| owned.get(name))
        .map(|registered| (registered.function, registered.context))
}

unsafe extern "C" fn host_log(host: *const ModHost, level: u32, message: *const c_char) {
    let Some(record) = (unsafe { record(host) }) else {
        return;
    };
    if message.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    let mod_id = record.mod_id.as_str();
    match level {
        0 => tracing::trace!(target: "shroudforge::mod", mod_id, "{message}"),
        1 => tracing::debug!(target: "shroudforge::mod", mod_id, "{message}"),
        2 => tracing::info!(target: "shroudforge::mod", mod_id, "{message}"),
        3 => tracing::warn!(target: "shroudforge::mod", mod_id, "{message}"),
        _ => tracing::error!(target: "shroudforge::mod", mod_id, "{message}"),
    }
}

unsafe extern "C" fn host_register_function(
    host: *const ModHost,
    name: *const c_char,
    function: Option<NativeFunction>,
    context: *mut c_void,
) -> u32 {
    let (Some(record), Some(function)) = (unsafe { record(host) }, function) else {
        return 2;
    };
    if name.is_null() {
        return 2;
    }
    let Some(name) = unsafe { CStr::from_ptr(name) }
        .to_str()
        .ok()
        .filter(|name| valid_function_name(name))
    else {
        return 2;
    };
    let mut functions = functions();
    if !record.accepting.load(Ordering::Acquire) {
        return 4;
    }
    let owned = functions.entry(record.mod_id.clone()).or_default();
    if owned.contains_key(name) {
        return 3;
    }
    owned.insert(
        name.to_owned(),
        Registered {
            function,
            context: context as usize,
            host: host as usize,
        },
    );
    tracing::debug!(target: "shroudforge::dll_manager", mod_id = %record.mod_id, name,
        "Native function registered for the mod's Lua code");
    1
}
