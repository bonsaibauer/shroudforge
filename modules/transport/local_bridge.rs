//! One-shot local UI-to-runtime handoff. Secrets never enter a settings file.

use serde_json::Value;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_LOCAL_MESSAGE_BYTES: usize = crate::MAX_MESSAGE_BYTES;
const MAX_LOCAL_REQUEST_QUEUE: usize = 32;
const MAX_LOCAL_REQUEST_BATCH: usize = 8;
// The P2P layer expires requests after 30 seconds. Give it five seconds to
// deliver that definitive result before the local bridge reports its own timeout.
const LOCAL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(35);

pub(crate) struct Request {
    pub(crate) value: Value,
    pub(crate) response: Sender<Value>,
}

static REQUESTS: OnceLock<Mutex<Receiver<Request>>> = OnceLock::new();

pub(crate) fn start() {
    let _ = REQUESTS.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel(MAX_LOCAL_REQUEST_QUEUE);
        #[cfg(windows)]
        std::thread::spawn(move || serve(sender, std::process::id()));
        #[cfg(not(windows))]
        let _ = sender;
        Mutex::new(receiver)
    });
}

pub(crate) fn drain() -> Vec<Request> {
    let Some(receiver) = REQUESTS.get() else {
        return Vec::new();
    };
    let Ok(receiver) = receiver.lock() else {
        return Vec::new();
    };
    receiver.try_iter().take(MAX_LOCAL_REQUEST_BATCH).collect()
}

pub fn submit(root: &std::path::Path, value: &Value) -> Result<Value, String> {
    let runtime_pid = shroudforge_loader_config::runtime_status::runtime_pid(root)
        .ok_or("The local Enshrouded client runtime is not running")?;
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_LOCAL_MESSAGE_BYTES {
        return Err("local Modloader request exceeds its size limit".into());
    }
    let request_id = value["requestId"].as_str().unwrap_or("unknown").to_owned();
    let request_kind = request_kind(value);
    let started = Instant::now();
    tracing::debug!(
        target: "shroudforge::network",
        request_id = %request_id,
        request_kind = %request_kind,
        "Remote UI request accepted by the local bridge"
    );
    #[cfg(windows)]
    {
        let result = request(runtime_pid, &bytes);
        match &result {
            Ok(response) => tracing::debug!(
                target: "shroudforge::network",
                request_id = %request_id,
                request_kind = %request_kind,
                success = response["success"] == true,
                authorized = response["authorized"]
                    .as_bool()
                    .unwrap_or(response["success"] == true),
                elapsed_ms = started.elapsed().as_millis() as u64,
                "Remote UI result returned to the local Modloader UI"
            ),
            Err(error) => tracing::warn!(
                target: "shroudforge::network",
                request_id = %request_id,
                request_kind = %request_kind,
                elapsed_ms = started.elapsed().as_millis() as u64,
                %error,
                "Local bridge request failed before returning a server result"
            ),
        }
        result
    }
    #[cfg(not(windows))]
    {
        let _ = (runtime_pid, bytes, request_id, request_kind, started);
        Err("local Modloader runtime requests are supported only on Windows".into())
    }
}

fn request_kind(value: &Value) -> String {
    match value["type"].as_str().unwrap_or("unknown") {
        "mod-operation" => format!(
            "mod-operation/{}",
            value["operation"].as_str().unwrap_or("unknown")
        ),
        request_type => request_type.to_owned(),
    }
}

#[cfg(windows)]
fn pipe_name(runtime_pid: u32) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(&format!(r"\\.\pipe\ShroudForge.RemoteUI.{runtime_pid}"))
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn request(runtime_pid: u32, bytes: &[u8]) -> Result<Value, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING, WriteFile};
    use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
    let name = pipe_name(runtime_pid);
    if unsafe { WaitNamedPipeW(name.as_ptr(), 5000) } == 0 {
        return Err("Could not reach the local Enshrouded runtime".into());
    }
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            windows_sys::Win32::Foundation::GENERIC_READ
                | windows_sys::Win32::Foundation::GENERIC_WRITE,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let result = (|| {
        let mut frame = Vec::with_capacity(bytes.len() + 4);
        frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        frame.extend_from_slice(bytes);
        let mut written = 0;
        if unsafe {
            WriteFile(
                handle,
                frame.as_ptr(),
                frame.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            )
        } == 0
            || written as usize != frame.len()
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut length = [0u8; 4];
        read_exact(handle, &mut length)?;
        let response_len = u32::from_le_bytes(length) as usize;
        if response_len == 0 || response_len > MAX_LOCAL_MESSAGE_BYTES {
            return Err("invalid local Modloader response size".into());
        }
        let mut response = vec![0u8; response_len];
        read_exact(handle, &mut response)?;
        serde_json::from_slice(&response).map_err(|error| error.to_string())
    })();
    unsafe { CloseHandle(handle) };
    result
}

#[cfg(windows)]
fn read_exact(
    handle: windows_sys::Win32::Foundation::HANDLE,
    mut output: &mut [u8],
) -> Result<(), String> {
    use windows_sys::Win32::Storage::FileSystem::ReadFile;
    while !output.is_empty() {
        let mut read = 0;
        if unsafe {
            ReadFile(
                handle,
                output.as_mut_ptr(),
                output.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        } == 0
            || read == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        output = &mut output[read as usize..];
    }
    Ok(())
}

#[cfg(windows)]
fn serve(sender: SyncSender<Request>, runtime_pid: u32) {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, FlushFileBuffers, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_MESSAGE, PIPE_WAIT,
    };
    const ERROR_PIPE_CONNECTED: u32 = 535;
    let name = pipe_name(runtime_pid);
    let mut first_instance = true;
    let mut last_create_error_log = None;
    loop {
        let open_mode = PIPE_ACCESS_DUPLEX
            | if first_instance {
                FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                0
            };
        let pipe = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                open_mode,
                PIPE_TYPE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS | PIPE_WAIT,
                1,
                MAX_LOCAL_MESSAGE_BYTES as u32,
                MAX_LOCAL_MESSAGE_BYTES as u32,
                5000,
                std::ptr::null(),
            )
        };
        if pipe == INVALID_HANDLE_VALUE {
            let error = std::io::Error::last_os_error();
            if last_create_error_log
                .is_none_or(|last: Instant| last.elapsed() >= Duration::from_secs(30))
            {
                tracing::warn!(target: "shroudforge::network", %error, "Could not create local Modloader request pipe");
                last_create_error_log = Some(Instant::now());
            }
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        if last_create_error_log.take().is_some() {
            tracing::info!(target: "shroudforge::network", "Local Modloader request pipe is available again");
        }
        first_instance = false;
        let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if connected {
            let mut frame = vec![0u8; MAX_LOCAL_MESSAGE_BYTES + 4];
            let mut read = 0;
            let received = unsafe {
                ReadFile(
                    pipe,
                    frame.as_mut_ptr(),
                    frame.len() as u32,
                    &mut read,
                    std::ptr::null_mut(),
                )
            } != 0;
            let response = if received && read >= 4 {
                let len = u32::from_le_bytes(frame[..4].try_into().unwrap()) as usize;
                if len == read as usize - 4 && len > 0 && len <= MAX_LOCAL_MESSAGE_BYTES {
                    match serde_json::from_slice::<Value>(&frame[4..read as usize]) {
                        Ok(value) => dispatch(sender.clone(), value),
                        Err(error) => {
                            serde_json::json!({"success":false,"message":error.to_string()})
                        }
                    }
                } else {
                    serde_json::json!({"success":false,"message":"invalid local request frame"})
                }
            } else {
                serde_json::json!({"success":false,"message":"could not read local request"})
            };
            if let Ok(bytes) = serde_json::to_vec(&response)
                && bytes.len() <= MAX_LOCAL_MESSAGE_BYTES
            {
                let mut response_frame = Vec::with_capacity(bytes.len() + 4);
                response_frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                response_frame.extend_from_slice(&bytes);
                let mut written = 0;
                unsafe {
                    WriteFile(
                        pipe,
                        response_frame.as_ptr(),
                        response_frame.len() as u32,
                        &mut written,
                        std::ptr::null_mut(),
                    );
                    FlushFileBuffers(pipe);
                }
            }
        }
        unsafe {
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
        }
    }
}

#[cfg(windows)]
fn dispatch(sender: SyncSender<Request>, value: Value) -> Value {
    if !matches!(
        value["type"].as_str(),
        Some(
            "admin-login"
                | "config-setting"
                | "mods-list"
                | "mod-operation"
                | "server-rights-list"
                | "server-rights-update"
        )
    ) || value["requestId"]
        .as_str()
        .is_none_or(|request_id| request_id.is_empty() || request_id.len() > 128)
    {
        return serde_json::json!({"success":false,"message":"unsupported local request"});
    }
    if value["type"] == "admin-login"
        && value["password"]
            .as_str()
            .is_none_or(|password| password.is_empty() || password.len() > 1024)
    {
        return serde_json::json!({"success":false,"message":"password must contain 1 to 1024 bytes"});
    }
    if value["type"] == "config-setting"
        && (!value["expectedModules"].is_object() || !value["modules"].is_object())
    {
        return serde_json::json!({"success":false,"message":"invalid server settings update"});
    }
    if value["type"] == "mods-list"
        && value["locale"]
            .as_str()
            .is_some_and(|locale| locale.is_empty() || locale.len() > 32)
    {
        return serde_json::json!({"success":false,"message":"invalid locale"});
    }
    if value["type"] == "server-rights-update" {
        let Some(groups) = value["values"]["groups"].as_array() else {
            return serde_json::json!({"success":false,"message":"invalid server rights update"});
        };
        let Some(expected_groups) = value["values"]["expectedGroups"].as_array() else {
            return serde_json::json!({"success":false,"message":"invalid server rights update"});
        };
        let invalid_groups = |list: &[Value]| {
            list.len() > 64 || list.iter().any(|group| {
                group["name"].as_str().is_none_or(|name| {
                    name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control)
                }) || [
                    "canKickBan",
                    "canAccessInventories",
                    "canEditWorld",
                    "canEditBase",
                    "canExtendBase",
                    "syncServerLogs",
                ]
                .iter()
                .any(|key| !group[*key].is_boolean())
            })
        };
        if invalid_groups(groups) || invalid_groups(expected_groups) {
            return serde_json::json!({"success":false,"message":"invalid server rights update"});
        }
    }
    if value["type"] == "mod-operation" {
        let operation = value["operation"].as_str();
        if value["modId"]
            .as_str()
            .is_none_or(|id| !mod_loader::package_lookup::valid_identifier(id))
        {
            return serde_json::json!({"success":false,"message":"invalid mod identifier"});
        }
        let valid = match operation {
            Some("save-settings") => {
                value["values"].is_object()
                    && value["revision"]
                        .as_str()
                        .is_some_and(|revision| !revision.is_empty() && revision.len() <= 128)
            }
            Some("set-enabled") => {
                value["enabled"].is_boolean()
                    && value["revision"]
                        .as_str()
                        .is_some_and(|revision| !revision.is_empty() && revision.len() <= 128)
            }
            Some("key-reservation") => {
                value["key"]
                    .as_str()
                    .is_some_and(|key| !key.is_empty() && key.len() <= 128)
                    && matches!(value["device"].as_str(), Some("keyboard" | "mouse"))
                    && value["reserved"].is_boolean()
            }
            Some("run-action") => value["action"]
                .as_str()
                .is_some_and(|action| mod_loader::package_lookup::valid_identifier(action)),
            Some("assess-hotkey") => {
                value["key"]
                    .as_str()
                    .is_some_and(|key| !key.is_empty() && key.len() <= 128)
                    && value["values"].is_object()
            }
            _ => false,
        };
        if !valid {
            return serde_json::json!({"success":false,"message":"invalid server mod request"});
        }
    }
    let (response, receive) = mpsc::channel();
    let request_id = value["requestId"].as_str().unwrap_or("unknown").to_owned();
    let request_kind = request_kind(&value);
    match sender.try_send(Request { value, response }) {
        Ok(()) => tracing::debug!(
            target: "shroudforge::network",
            request_id = %request_id,
            request_kind = %request_kind,
            "Remote UI request entered the runtime queue"
        ),
        Err(TrySendError::Full(_)) => {
            return serde_json::json!({
                "success": false,
                "message": "too many local Modloader requests are queued"
            });
        }
        Err(TrySendError::Disconnected(_)) => {
            return serde_json::json!({
                "success": false,
                "message": "local runtime request queue is unavailable"
            });
        }
    }
    match receive.recv_timeout(LOCAL_RESPONSE_TIMEOUT) {
        Ok(response) => {
            tracing::debug!(
                target: "shroudforge::network",
                request_id = %request_id,
                request_kind = %request_kind,
                success = response["success"] == true,
                authorized = response["authorized"]
                    .as_bool()
                    .unwrap_or(response["success"] == true),
                "Runtime queue completed local remote UI request"
            );
            response
        }
        Err(_) => {
            tracing::warn!(
                target: "shroudforge::network",
                request_id = %request_id,
                request_kind = %request_kind,
                timeout_seconds = LOCAL_RESPONSE_TIMEOUT.as_secs(),
                "Local bridge timed out waiting for the P2P result. Server action outcome may be unknown"
            );
            serde_json::json!({"success":false,"message":"server response timed out; refresh the server state before retrying"})
        }
    }
}
