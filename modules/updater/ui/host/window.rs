#![cfg(windows)]

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use tao::platform::windows::WindowBuilderExtWindows;
use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{WebContext, WebViewBuilder};

pub fn run(root: Option<PathBuf>) -> Result<(), String> {
    let selected = root
        .filter(|path| is_game_root(path))
        .or_else(installed_root);
    let mut window_mutex = if let Some(selected) = selected.as_ref() {
        shroudforge_loader_config::config::initialize_loader_config(selected)?;
        shroudforge_loader_config::config::request_window_visibility(selected, "updates", true)?;
        let Some(mutex) = acquire_window_mutex(selected)? else {
            return Ok(());
        };
        Some((selected.clone(), mutex))
    } else {
        None
    };
    let mut visibility_request_id = selected
        .as_ref()
        .map(|path| shroudforge_loader_config::config::window_state(path))
        .and_then(|state| state["updates"]["requestId"].as_u64())
        .unwrap_or(0);
    let root = Arc::new(Mutex::new(selected.clone()));
    let (command_sender, command_receiver) = mpsc::channel::<serde_json::Value>();
    let (result_sender, result_receiver) = mpsc::channel::<serde_json::Value>();
    let check_state = Arc::new(Mutex::new(String::new()));
    let mod_check_running = Arc::new(Mutex::new(false));
    let event_loop = EventLoopBuilder::<()>::with_user_event().build();
    let (icon_rgba, icon_width, icon_height) =
        shroudforge_ui_resources::rgba().map_err(|error| error.to_string())?;
    let icon = tao::window::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .map_err(|error| error.to_string())?;
    let window = WindowBuilder::new()
        .with_window_icon(Some(icon.clone()))
        .with_taskbar_icon(Some(icon))
        .with_title("ShroudForge Updater")
        .with_resizable(true)
        .with_inner_size(LogicalSize::new(960.0, 720.0))
        .build(&event_loop)
        .map_err(|error| error.to_string())?;
    let profile = root.lock().ok().and_then(|root| root.as_ref().cloned());
    let profile = profile
        .map(|root| shroudforge_loader_config::paths::updates_dir(&root).join("webview-profile"));
    if let Some(profile) = &profile {
        std::fs::create_dir_all(profile).map_err(|error| error.to_string())?;
    }
    let mut context = WebContext::new(profile);
    if let Some(root) = root.lock().ok().and_then(|root| root.clone()) {
        let _ =
            shroudforge_logging::append(&root, 'I', "updater", "Standalone updater window opened");
    }
    let sender = command_sender.clone();
    let handler = move |request: wry::http::Request<String>| {
        if let Ok(command) = serde_json::from_str(request.body()) {
            let _ = sender.send(command);
        }
    };
    let html = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../build/ui/updater/updater.html"
    ));
    let webview = shroudforge_ui_resources::with_embedded_document(
        WebViewBuilder::new_with_web_context(&mut context),
        html.as_bytes(),
    )
    .with_ipc_handler(handler)
    .build(&window)
    .map_err(|error| error.to_string())?;
    if let Some(root) = root.lock().ok().and_then(|root| root.clone()) {
        let _ =
            shroudforge_loader_config::config::publish_window_visibility(&root, "updates", true);
    }

    let mut next_refresh = Instant::now();
    let mut next_visibility_poll = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            let selected_root = match root.lock() {
                Ok(root) => root.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            };
            if let Some(root) = selected_root {
                if let Err(error) = cancel_running_queue_before_close(&root) {
                    let message = format!(
                        "The update queue could not be cancelled safely. The updater window stayed open: {error}"
                    );
                    let _ = shroudforge_logging::append(
                        &root,
                        'E',
                        "updater",
                        &message,
                    );
                    send_event(
                        &webview,
                        "__shroudforgeUpdaterResult",
                        &serde_json::json!({"error": message}),
                    );
                    *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
                    return;
                }
                let _ = shroudforge_loader_config::config::publish_window_visibility(
                    &root, "updates", false,
                );
            }
            *control_flow = ControlFlow::Exit;
            return;
        }
        if matches!(event, Event::MainEventsCleared)
            || matches!(event, Event::NewEvents(StartCause::Init))
            || matches!(event, Event::NewEvents(StartCause::ResumeTimeReached { .. }))
        {
            while let Ok(command) = command_receiver.try_recv() {
                let exit = handle_command(
                    &command,
                    &root,
                    &mut window_mutex,
                    &mut visibility_request_id,
                    &check_state,
                    &mod_check_running,
                    &result_sender,
                );
                if exit {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
            while let Ok(result) = result_receiver.try_recv() {
                send_event(&webview, "__shroudforgeUpdaterResult", &result);
            }
            if Instant::now() >= next_visibility_poll {
                if let Some(root) = root.lock().ok().and_then(|root| root.clone()) {
                    let state = shroudforge_loader_config::config::window_state(&root);
                    let request_id = state["updates"]["requestId"]
                        .as_u64()
                        .unwrap_or(visibility_request_id);
                    if request_id != visibility_request_id {
                        visibility_request_id = request_id;
                        let visible = state["updates"]["requestedVisible"]
                            .as_bool()
                            .unwrap_or(true);
                        window.set_visible(visible);
                        if visible {
                            window.set_minimized(false);
                            window.set_focus();
                        }
                        let _ = shroudforge_loader_config::config::publish_window_visibility(
                            &root, "updates", visible,
                        );
                        shroudforge_logging::write_activity(
                            &root,
                            "Updates",
                            if visible {
                                "Show updater window"
                            } else {
                                "Hide updater window"
                            },
                            "Succeeded",
                            None,
                            "info",
                        );
                    }
                }
                next_visibility_poll = Instant::now() + Duration::from_millis(100);
            }
            if Instant::now() >= next_refresh {
                if let Some(root) = root.lock().ok().and_then(|root| root.clone()) {
                    let snapshot = read_snapshot(&root, &check_state, &mod_check_running);
                    send_event(&webview, "__shroudforgeUpdaterSnapshot", &snapshot);
                } else {
                    let snapshot = serde_json::json!({
                        "root": "",
                        "suggestedRoot": default_game_root().to_string_lossy(),
                        "installationRootSelected": false,
                        "version": env!("CARGO_PKG_VERSION"),
                        "release": {"currentVersion": env!("CARGO_PKG_VERSION"), "installationDetected": false, "state":"idle", "message":"Select the Enshrouded installation folder to continue."},
                        "queue": {"items": [], "run": null},
                    });
                    send_event(&webview, "__shroudforgeUpdaterSnapshot", &snapshot);
                }
                next_refresh = Instant::now() + Duration::from_millis(700);
            }
        }
    });
}

fn cancel_running_queue_before_close(root: &Path) -> Result<bool, String> {
    let queue = crate::try_read_update_queue_value(root);
    if queue
        .as_ref()
        .is_ok_and(|queue| queue["run"]["state"] != "running")
    {
        return Ok(false);
    }
    if queue
        .as_ref()
        .is_ok_and(|queue| queue["run"]["installing"] == true && queue["run"]["cancelable"] != true)
    {
        return Ok(false);
    }
    let run_token = queue
        .as_ref()
        .ok()
        .and_then(|queue| crate::update_queue_storage::update_queue_run_token(&queue["run"]));
    // Signal through the cancellation file before touching the queue lock.
    // The independent worker polls this file in downloads, extraction, copies,
    // installation and rollback. The window must stay responsive if a worker
    // is currently waiting on the shared installation lock.
    crate::scheduled::clear_system_update(root, run_token.as_deref())?;
    let (level, message) = match queue {
        Ok(_) => (
            'I',
            "Updater window close requested. Active update queue cancellation was signalled"
                .to_owned(),
        ),
        Err(error) => (
            'W',
            format!(
                "Updater window close requested. The update queue could not be read ({error}), but cancellation was signalled"
            ),
        ),
    };
    let _ = shroudforge_logging::append(root, level, "updater", &message);
    Ok(true)
}

struct UpdaterWindowMutex(windows_sys::Win32::Foundation::HANDLE);

impl Drop for UpdaterWindowMutex {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

fn acquire_window_mutex(root: &Path) -> Result<Option<UpdaterWindowMutex>, String> {
    use sha2::Digest;
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::CreateMutexW,
    };

    let normalized_root = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    let name = format!(
        "Local\\ShroudForgeUpdaterWindow_{:x}",
        sha2::Sha256::digest(normalized_root.as_bytes())
    );
    let wide: Vec<u16> = OsStr::new(&name).encode_wide().chain(Some(0)).collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 1, wide.as_ptr()) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
        return Ok(None);
    }
    Ok(Some(UpdaterWindowMutex(handle)))
}

fn handle_command(
    command: &serde_json::Value,
    root: &Arc<Mutex<Option<PathBuf>>>,
    window_mutex: &mut Option<(PathBuf, UpdaterWindowMutex)>,
    visibility_request_id: &mut u64,
    check_state: &Arc<Mutex<String>>,
    mod_check_running: &Arc<Mutex<bool>>,
    results: &mpsc::Sender<serde_json::Value>,
) -> bool {
    let action = command["command"].as_str().unwrap_or_default();
    if action == "choose-root" {
        let initial_folder = suggested_game_picker_folder();
        match shroudforge_window_management::choose_folder_from(
            "Choose the Enshrouded installation folder",
            Some(&initial_folder),
        ) {
            Ok(Some(path)) if is_game_root(&path) => {
                let same_root = root
                    .lock()
                    .ok()
                    .and_then(|selected| {
                        selected.as_ref().map(|selected| same_path(selected, &path))
                    })
                    .unwrap_or(false);
                let new_mutex = if same_root {
                    None
                } else {
                    match acquire_window_mutex(&path) {
                        Ok(Some(mutex)) => Some(mutex),
                        Ok(None) => {
                            let _ = results.send(serde_json::json!({"error":"The updater window is already open for this game folder."}));
                            return false;
                        }
                        Err(error) => {
                            let _ = results.send(serde_json::json!({"error":error}));
                            return false;
                        }
                    }
                };
                if let Err(error) = shroudforge_loader_config::config::initialize_loader_config(
                    &path,
                )
                .and_then(|()| {
                    shroudforge_loader_config::config::request_window_visibility(
                        &path, "updates", true,
                    )
                }) {
                    let _ = results.send(serde_json::json!({"error":error}));
                    return false;
                }
                let state = shroudforge_loader_config::config::window_state(&path);
                *visibility_request_id = state["updates"]["requestId"].as_u64().unwrap_or(0);
                if let Some(mutex) = new_mutex {
                    *window_mutex = Some((path.clone(), mutex));
                }
                let _ = shroudforge_loader_config::config::publish_window_visibility(
                    &path, "updates", true,
                );
                if let Ok(mut selected) = root.lock() {
                    *selected = Some(path.clone());
                }
                let _ = shroudforge_logging::append(
                    &path,
                    'I',
                    "updater",
                    &format!("Installation selected: {}", path.display()),
                );
            }
            Ok(Some(_)) => {
                let _ = results.send(serde_json::json!({"error":"The selected folder is not an Enshrouded installation."}));
            }
            Ok(None) => {}
            Err(error) => {
                let _ = results.send(serde_json::json!({"error":error}));
            }
        }
        return false;
    }
    let Some(root_path) = root.lock().ok().and_then(|root| root.clone()) else {
        let _ = results
            .send(serde_json::json!({"error":"Select the Enshrouded installation folder first."}));
        return false;
    };
    let result = match action {
        "check-updates" => {
            let check_state = check_state.clone();
            let results = results.clone();
            let root = root_path.clone();
            let Ok(mut value) = check_state.lock() else {
                let _ = results
                    .send(serde_json::json!({"error":"Could not update the system check status."}));
                return false;
            };
            if *value == "checking" {
                return false;
            }
            *value = "checking".into();
            drop(value);
            std::thread::spawn(move || {
                let result = crate::run_request(
                    &root,
                    &serde_json::json!({"command":"check-system-update","manual":true}),
                );
                let message = match result {
                    Ok(value) => value,
                    Err(error) => serde_json::json!({"error":error}),
                };
                if let Ok(mut value) = check_state.lock() {
                    value.clear();
                }
                let _ = results.send(message);
            });
            Ok(())
        }
        "check-mod-updates" => {
            let running = mod_check_running.clone();
            let results = results.clone();
            let root = root_path.clone();
            let Ok(mut value) = running.lock() else {
                let _ = results
                    .send(serde_json::json!({"error":"Could not update the mod check status."}));
                return false;
            };
            if *value {
                return false;
            }
            *value = true;
            drop(value);
            std::thread::spawn(move || {
                let result = crate::run_request(
                    &root,
                    &serde_json::json!({"command":"check-mod-updates","manual":true}),
                );
                if let Ok(mut value) = running.lock() {
                    *value = false;
                }
                let message = match result {
                    Ok(value) => value,
                    Err(error) => serde_json::json!({"error":error}),
                };
                let _ = results.send(message);
            });
            Ok(())
        }
        "cancel-update" => {
            let root = root_path.clone();
            let results = results.clone();
            std::thread::spawn(move || {
                let result =
                    crate::run_request(&root, &serde_json::json!({"command":"cancel-update"}));
                let message = match result {
                    Ok(value) => value,
                    Err(error) => serde_json::json!({"error":error}),
                };
                let _ = results.send(message);
            });
            Ok(())
        }
        "open-url" => command["url"]
            .as_str()
            .ok_or_else(|| "updater link request is missing its URL".to_owned())
            .and_then(open_url)
            .map(|()| serde_json::json!({"opened":true}))
            .map(|_| ()),
        _ => {
            let request = updater_request(&root_path, command);
            crate::run_request(&root_path, &request).map(|_| ())
        }
    };
    match result {
        Ok(()) => false,
        Err(error) => {
            let _ = results.send(serde_json::json!({"error":error}));
            false
        }
    }
}

fn updater_request(root: &Path, command: &serde_json::Value) -> serde_json::Value {
    let mut request = command.clone();
    match command["command"].as_str().unwrap_or_default() {
        "queue-system-update" => request["command"] = "enqueue-system-update".into(),
        "remove-update-queue-item" => {
            request["id"] = command["key"].clone();
        }
        "search-catalog" | "install-mod" | "update-mod" => {
            let config = shroudforge_loader_config::config::read_loader(root).unwrap_or_default();
            request["provider"] =
                config
                    .pointer("/catalog/provider")
                    .cloned()
                    .unwrap_or_else(|| {
                        serde_json::to_value(
                            crate::provider_config::CatalogProviderConfig::default(),
                        )
                        .unwrap_or_default()
                    });
        }
        _ => {}
    }
    request
}

fn open_url(url: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    if !url.starts_with("https://") && url != "mailto:support@shroudedit.com" {
        return Err("Only HTTPS links and the ShroudEdit support address can be opened.".into());
    }
    let operation = std::ffi::OsStr::new("open")
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = std::ffi::OsStr::new(url)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        ) as isize
    };
    if result <= 32 {
        return Err(format!(
            "Windows could not open the link (ShellExecuteW returned {result})."
        ));
    }
    Ok(())
}

fn read_snapshot(
    root: &Path,
    check_state: &Arc<Mutex<String>>,
    mod_check_running: &Arc<Mutex<bool>>,
) -> serde_json::Value {
    let config = shroudforge_loader_config::config::read_loader(root).unwrap_or_default();
    let installed = crate::installed_release(root);
    let mut release = crate::release_state::ReleaseState::idle(
        installed,
        shroudforge_loader_config::paths::updates_dir(root)
            .join("pending.ready")
            .is_file(),
    );
    release.installation_detected =
        shroudforge_loader_config::paths::shroudforge_program_files_present(root);
    let state = Arc::new(Mutex::new(release.clone()));
    crate::restore_checked_release::restore_persisted_update_result_from_config(&config, &state);
    if let Ok(value) = state.lock() {
        release = value.clone();
    }
    if check_state.lock().is_ok_and(|value| *value == "checking") {
        release.state = "checking".into();
        release.message = Some("Checking GitHub releases…".into());
    }
    let status_path =
        shroudforge_loader_config::paths::updates_dir(root).join("updater-status.json");
    if let Ok(bytes) = std::fs::read(status_path)
        && let Ok(status) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && let Some(state) = status["status"].as_str()
        && matches!(
            state,
            "queued"
                | "downloading"
                | "verifying"
                | "extracting"
                | "waitingForGame"
                | "installing"
                | "restoring"
                | "uninstalling"
                | "cancelled"
                | "error"
                | "installed"
                | "uninstalled"
        )
    {
        release.state = state.to_owned();
        release.message = status["message"].as_str().map(str::to_owned);
        release.downloaded_bytes = status["downloadedBytes"].as_u64().unwrap_or_default();
        release.total_bytes = status["totalBytes"].as_u64();
        release.bytes_per_second = status["bytesPerSecond"].as_u64();
        release.completed_files = status["completedFiles"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok());
        release.total_files = status["totalFiles"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok());
        release.current_file = status["currentFile"].as_str().map(str::to_owned);
        release.current_file_bytes = status["currentFileBytes"].as_u64();
        release.current_file_total_bytes = status["currentFileTotalBytes"].as_u64();
    }
    if release.update_available
        && let Some(notes) = config
            .pointer("/modules/updates/system/latestRelease/message")
            .and_then(serde_json::Value::as_str)
    {
        release.message = Some(notes.to_owned());
    }
    serde_json::json!({
        "root": root.to_string_lossy(),
        "suggestedRoot": default_game_root().to_string_lossy(),
        "installationRootSelected": true,
        "version": release.current_version,
        "release": release,
        "queue": crate::read_update_queue(root),
        "backupManagement": shroudforge_game_assets::originals::management_status(
            root,
            if root.join("enshrouded_server.exe").is_file() { "server" } else { "client" },
        ),
        "modUpdates": config.pointer("/modules/updates/mods/latestUpdates").cloned().unwrap_or_else(|| serde_json::json!([])),
        "modCheckResult": config.pointer("/modules/updates/mods/lastCheckResult").cloned(),
        "modCheckRunning": mod_check_running.lock().is_ok_and(|value| *value),
        "updatesEnabled": config.pointer("/modules/updates/enabled").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "locale": config.pointer("/general/locale").and_then(serde_json::Value::as_str).unwrap_or("de")
    })
}

fn send_event(webview: &wry::WebView, name: &str, value: &serde_json::Value) {
    if let Ok(json) = serde_json::to_string(value) {
        let script = format!("window.{name}&&window.{name}({json})");
        let _ = webview.evaluate_script(&script);
    }
}

fn installed_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let module = executable.parent()?;
    let root = module.parent()?;
    is_game_root(root).then(|| root.to_path_buf())
}

pub(crate) fn is_game_root(root: &Path) -> bool {
    root.join("enshrouded.exe").is_file()
        || root.join("enshrouded_server.exe").is_file()
        || root.join("enshrouded_server").is_file()
        || root.join("enshrouded_server.x86_64").is_file()
}

fn default_game_root() -> PathBuf {
    let program_files = std::env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
    program_files.join("Steam/steamapps/common/Enshrouded")
}

fn suggested_game_picker_folder() -> PathBuf {
    let mut candidate = default_game_root();
    while !candidate.is_dir() {
        if !candidate.pop() {
            return PathBuf::from(r"C:\");
        }
    }
    candidate
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(&right.to_string_lossy().replace('/', "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "shroudforge-updater-close-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn closing_during_irreversible_install_does_not_signal_cancellation() {
        let root = temporary_root("installing");
        let updates = shroudforge_loader_config::paths::updates_dir(&root);
        std::fs::create_dir_all(&updates).unwrap();
        shroudforge_loader_config::config::write_json(
            &updates.join("update-queue.json"),
            &serde_json::json!({
                "schemaVersion": 1,
                "items": [{"id":"system:test","kind":"system","state":"installing"}],
                "run": {
                    "state":"running",
                    "installing":true,
                    "cancelable":false,
                    "cancelRequested":false,
                    "itemIds":["system:test"]
                }
            }),
        )
        .unwrap();

        assert!(!cancel_running_queue_before_close(&root).unwrap());
        crate::check_update_cancelled::check_update_cancelled(&root).unwrap();
        assert!(!updates.join("cancel-system-update").exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn closing_without_a_running_queue_does_not_create_a_cancellation() {
        let root = temporary_root("idle");

        assert!(!cancel_running_queue_before_close(&root).unwrap());
        assert!(
            !shroudforge_loader_config::paths::updates_dir(&root)
                .join("cancel-system-update")
                .exists()
        );
    }

    #[test]
    fn closing_with_a_corrupt_queue_still_signals_cancellation() {
        let root = temporary_root("corrupt");
        let updates = shroudforge_loader_config::paths::updates_dir(&root);
        std::fs::create_dir_all(&updates).unwrap();
        std::fs::write(updates.join("update-queue.json"), b"{").unwrap();

        assert!(cancel_running_queue_before_close(&root).unwrap());
        assert!(updates.join("cancel-system-update").is_file());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn closing_when_cancellation_cannot_be_written_keeps_the_window_open() {
        let root = temporary_root("unwritable-cancel");
        let updates = shroudforge_loader_config::paths::updates_dir(&root);
        std::fs::create_dir_all(&updates).unwrap();
        shroudforge_loader_config::config::write_json(
            &updates.join("update-queue.json"),
            &serde_json::json!({
                "schemaVersion": 1,
                "items": [],
                "run": {"state":"running", "runId":"active"}
            }),
        )
        .unwrap();
        std::fs::create_dir(updates.join("cancel-system-update")).unwrap();

        assert!(cancel_running_queue_before_close(&root).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }
}
