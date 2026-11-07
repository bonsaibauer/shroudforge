#[path = "asset_preparation/mod.rs"]
mod pregame;
#[cfg(any(windows, target_os = "linux"))]
#[path = "start_updater_controller.rs"]
mod start_updater_controller;
#[cfg(test)]
mod tests;

use shroudforge_api::{IngameRuntime, ShroudForgeApi};
use std::any::Any;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use pregame::run as prepare;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("loader environment error: {0}")]
    Environment(String),
    #[error("pregame phase failed: {0}")]
    Pregame(String),
    #[error("runtime initialization failed: {0}")]
    Runtime(String),
}

#[derive(Debug, Clone, Default)]
pub struct LoadReport {
    pub lua: Vec<String>,
}

/// Run update-versioned migrations with the installed updater before the loader
/// reads data that the update may have changed.
#[cfg(windows)]
pub fn run_post_install_migrations(root: &Path) -> Result<(), String> {
    use std::{io::Write, os::windows::process::CommandExt, process::Stdio, time::Duration};

    let executable = shroudforge_loader_config::paths::updater_executable(root);
    if !executable.is_file() {
        return Ok(());
    }
    let request = serde_json::to_vec(&serde_json::json!({
        "command": "run-post-install-maintenance"
    }))
    .map_err(|error| format!("could not encode post-install maintenance request: {error}"))?;

    for attempt in 0..40 {
        let mut child = std::process::Command::new(&executable)
            .args(["--request", "--root"])
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|error| format!("could not start the installed updater: {error}"))?;
        child
            .stdin
            .take()
            .ok_or("installed updater request stdin is unavailable")?
            .write_all(&request)
            .map_err(|error| format!("could not send post-install maintenance request: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("could not read post-install maintenance result: {error}"))?;
        let response: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("installed updater returned an invalid response: {error}"))?;
        if output.status.success() && response["ok"] == true {
            return Ok(());
        }
        let error = response["error"]
            .as_str()
            .unwrap_or("installed updater could not finish post-install maintenance");
        let worker_is_busy = error.contains("post-install maintenance was deferred")
            || error.contains("previous update queue finishes");
        if !worker_is_busy || attempt == 39 {
            let message = format!("post-install migration failed: {error}");
            let _ = shroudforge_logging::append(root, 'E', "updater", &message);
            return Err(message);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn run_post_install_migrations(_: &Path) -> Result<(), String> {
    Ok(())
}

pub(crate) fn recycle_incompatible_game_asset_backups(
    root: &Path,
    target: &str,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::{io::Write, os::windows::process::CommandExt, process::Stdio};

        let request = serde_json::json!({
            "command": "recycle-game-asset-backups",
            "target": target,
        });
        let executable = shroudforge_loader_config::paths::updater_executable(root);
        if !executable.is_file() {
            return Err(
                "ShroudForge Updater is unavailable, so the old asset backups were kept".into(),
            );
        }
        let mut child = std::process::Command::new(executable)
            .args(["--request", "--root"])
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|error| format!("could not start ShroudForge Updater: {error}"))?;
        child
            .stdin
            .take()
            .ok_or("ShroudForge Updater request stdin is unavailable")?
            .write_all(&serde_json::to_vec(&request).map_err(|error| error.to_string())?)
            .map_err(|error| format!("could not send recycle-bin request: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("could not read recycle-bin result: {error}"))?;
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                format!("ShroudForge Updater returned an invalid response: {error}")
            })?;
        if output.status.success() && response["ok"] == true {
            Ok(())
        } else {
            Err(response["error"]
                .as_str()
                .unwrap_or("could not move the incompatible asset backups to the recycle bin")
                .to_owned())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (root, target);
        Err("moving incompatible game asset backups to the recycle bin is available on Windows only".into())
    }
}

pub struct ModLoader {
    api: Option<ShroudForgeApi>,
    runtime: Option<IngameRuntime>,
    is_client: bool,
}

impl ModLoader {
    pub fn new(api: ShroudForgeApi, is_client: bool) -> Self {
        Self {
            api: Some(api),
            runtime: None,
            is_client,
        }
    }

    pub fn load_directory(&mut self, game: impl AsRef<Path>) -> Result<LoadReport, LoaderError> {
        let game = game.as_ref();
        let root = game
            .to_str()
            .ok_or_else(|| LoaderError::Environment("game path is not UTF-8".into()))?;
        let environment = shroudforge_package::ModEnvironment::load(root)
            .map_err(|report| LoaderError::Environment(format!("{report:?}")))?;
        let file_name = if self.is_client {
            "enshrouded"
        } else {
            "enshrouded_server"
        };
        let runtime = match &self.api {
            Some(api) => IngameRuntime::start(&environment, api.clone(), file_name),
            None => IngameRuntime::start_live(&environment, file_name),
        }
        .map_err(|error| LoaderError::Runtime(error.to_string()))?;
        let report = LoadReport {
            lua: runtime.active_mod_ids(),
        };
        self.runtime = Some(runtime);
        Ok(report)
    }

    pub fn update(&mut self, delta_seconds: f64) {
        if let Some(runtime) = &mut self.runtime {
            runtime.update(delta_seconds);
        }
    }
}

#[repr(C)]
pub struct RuntimeHandle(ModLoader);

/// Receives native game-profile discovery messages and appends them to the
/// same process log that the Debug Console reads. This optional host export is
/// independent of the KFC runtime provider ABI.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_game_profile_discovery_log(
    level: u8,
    game_root: *const u16,
    message: *const std::ffi::c_char,
) {
    use std::{
        ffi::{CStr, OsString},
        os::windows::ffi::OsStringExt,
        path::PathBuf,
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if game_root.is_null()
            || message.is_null()
            || !matches!(level, b'T' | b'D' | b'I' | b'W' | b'E')
        {
            return;
        }
        let mut length = 0usize;
        unsafe {
            while length < 32768 && *game_root.add(length) != 0 {
                length += 1;
            }
        }
        if length == 0 || length == 32768 {
            return;
        }
        let root = PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(game_root, length)
        }));
        let detail = unsafe { CStr::from_ptr(message) }.to_string_lossy();
        let _ =
            shroudforge_logging::append(root, char::from(level), "game-profile-discovery", &detail);
    }));
}

fn panic_detail(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic payload was not a string".to_owned()
    }
}

fn prepare_native_dlls_before_game_entry(game: &Path) -> bool {
    let is_server = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .is_some_and(|name| name.eq_ignore_ascii_case("enshrouded_server.exe"));
    match std::panic::catch_unwind(|| shroudforge_api::prepare_native_dlls(game, is_server)) {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            let _ = shroudforge_logging::append(game, 'E', "dll-manager", &error);
            false
        }
        Err(payload) => {
            let _ = shroudforge_logging::append(
                game,
                'E',
                "dll-manager",
                &format!("Native DLL preparation failed: {}", panic_detail(&*payload)),
            );
            false
        }
    }
}

/// Applies stale pregame asset mods from the bootstrap thread before the live
/// ECS runtime is created. This lets a normal game start use the bundled
/// loader instead of requiring a separate `prepare` command.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_prepare_startup(game: *const u16) -> bool {
    fn path(value: *const u16) -> Option<PathBuf> {
        if value.is_null() {
            return None;
        }
        let mut length = 0;
        unsafe {
            while *value.add(length) != 0 {
                length += 1;
            }
        }
        Some(PathBuf::from(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(value, length)
        })))
    }
    match std::panic::catch_unwind(|| {
        let Some(game) = path(game) else {
            return false;
        };
        let _ = shroudforge_logging::initialize(&game, false);
        let started = std::time::Instant::now();
        let migrations_ready = match run_post_install_migrations(&game) {
            Ok(()) => true,
            Err(error) => {
                let _ = shroudforge_logging::append(
                    &game,
                    'E',
                    "updater",
                    &format!("Post-install migrations failed before startup preparation: {error}"),
                );
                false
            }
        };
        let startup_result = pregame::run_startup(&game);
        #[cfg(any(windows, target_os = "linux"))]
        if migrations_ready && let Err(error) = start_updater_controller::start(&game) {
            let _ = shroudforge_logging::append(
                &game,
                'E',
                "config-controller",
                &format!("Could not start headless config controller: {error}"),
            );
        }
        let assets_ready = match startup_result {
            Ok(()) => {
                let message = format!(
                    "Early startup asset pass completed in {} ms",
                    started.elapsed().as_millis()
                );
                let _ = shroudforge_logging::append(&game, 'I', "startup-assets", &message);
                true
            }
            Err(error) => {
                let _ =
                    shroudforge_logging::append(&game, 'W', "startup-assets", &error.to_string());
                pregame::recover_failed_startup(&game, &error.to_string())
            }
        };
        assets_ready && prepare_native_dlls_before_game_entry(&game)
    }) {
        Ok(result) => result,
        Err(payload) => {
            if let Some(game) = path(game) {
                let _ = shroudforge_logging::initialize(&game, false);
                let _ = shroudforge_logging::append(
                    &game,
                    'E',
                    "startup-assets",
                    &format!(
                        "Rust panic during startup asset preparation: {}",
                        panic_detail(&*payload)
                    ),
                );
                let assets_ready = pregame::recover_failed_startup(
                    &game,
                    &format!(
                        "Rust panic during startup asset preparation: {}",
                        panic_detail(&*payload)
                    ),
                );
                return assets_ready && prepare_native_dlls_before_game_entry(&game);
            }
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_create(game: *const u16) -> *mut RuntimeHandle {
    fn path(value: *const u16) -> Option<PathBuf> {
        if value.is_null() {
            return None;
        }
        let mut length = 0;
        unsafe {
            while *value.add(length) != 0 {
                length += 1;
            }
        }
        Some(PathBuf::from(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(value, length)
        })))
    }
    fn append_runtime_log(game: &Path, message: &str) {
        let _ = shroudforge_logging::append(game, 'E', "runtime", message);
    }
    fn create_runtime(game: PathBuf) -> Result<RuntimeHandle, String> {
        let _ = shroudforge_logging::initialize(&game, false);
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let name = executable
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let is_client = if name.eq_ignore_ascii_case("enshrouded.exe") {
            true
        } else if name.eq_ignore_ascii_case("enshrouded_server.exe") {
            false
        } else {
            return Err(format!("unsupported host process: {name}"));
        };
        let mut loader = ModLoader {
            api: None,
            runtime: None,
            is_client,
        };
        loader
            .load_directory(&game)
            .map_err(|error| format!("mod loading failed at {}: {error}", game.display()))?;
        Ok(RuntimeHandle(loader))
    }
    match std::panic::catch_unwind(|| {
        let Some(game) = path(game) else {
            return std::ptr::null_mut();
        };
        match create_runtime(game.clone()) {
            Ok(handle) => Box::into_raw(Box::new(handle)),
            Err(error) => {
                append_runtime_log(&game, &format!("Runtime initialization failed: {error}"));
                std::ptr::null_mut()
            }
        }
    }) {
        Ok(handle) => handle,
        Err(payload) => {
            if let Some(game) = path(game) {
                let _ = shroudforge_logging::initialize(&game, false);
                append_runtime_log(
                    &game,
                    &format!(
                        "Rust panic during runtime initialization: {}",
                        panic_detail(&*payload)
                    ),
                );
            }
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_update(
    handle: *mut RuntimeHandle,
    delta_seconds: f64,
) -> bool {
    if handle.is_null() {
        return false;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        (*handle).0.update(delta_seconds)
    }))
    .is_ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_destroy(handle: *mut RuntimeHandle) {
    if !handle.is_null() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(unsafe { Box::from_raw(handle) })
        }));
    }
}
