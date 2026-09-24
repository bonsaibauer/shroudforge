pub mod manifest;
mod pregame;

use shroudforge_api::{IngameRuntime, ShroudForgeApi};
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

    pub fn load_directory(&mut self, mods: impl AsRef<Path>) -> Result<LoadReport, LoaderError> {
        let game = mods
            .as_ref()
            .parent()
            .ok_or_else(|| LoaderError::Environment("mods path has no parent".into()))?;
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

/// Applies stale pregame asset mods from the bootstrap thread before the live
/// ECS runtime is created. This lets a normal game start use the bundled
/// loader instead of requiring a separate `prepare` command.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_prepare_startup(game: *const u16) -> bool {
    fn path(value: *const u16) -> Option<PathBuf> {
        if value.is_null() { return None; }
        let mut length = 0;
        unsafe { while *value.add(length) != 0 { length += 1; } }
        Some(PathBuf::from(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(value, length)
        })))
    }
    std::panic::catch_unwind(|| {
        let Some(game) = path(game) else { return false; };
        let _ = shroudforge_package::logging::initialize(&game, false);
        let started = std::time::Instant::now();
        match pregame::run_startup(&game) {
            Ok(()) => {
                let message = format!("Early startup asset pass completed in {} ms", started.elapsed().as_millis());
                let _ = shroudforge_package::logging::append(&game, 'I', "startup-assets", &message);
                true
            }
            Err(error) => {
                let _ = shroudforge_package::logging::append(
                    &game, 'W', "startup-assets", &error.to_string(),
                );
                false
            }
        }
    }).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn shroudforge_create(
    game: *const u16,
    mods: *const u16,
) -> *mut RuntimeHandle {
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
        let _ = shroudforge_package::logging::append(game, 'E', "runtime", message);
    }
    fn create_runtime(game: PathBuf, mods: PathBuf) -> Result<RuntimeHandle, String> {
        let _ = shroudforge_package::logging::initialize(&game, false);
        for error in shroudforge_package::migration::migrate_installation(&game)? {
            let _ = shroudforge_package::logging::append(&game, 'W', "migration", &error);
        }
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
            .load_directory(&mods)
            .map_err(|error| format!("mod loading failed at {}: {error}", mods.display()))?;
        Ok(RuntimeHandle(loader))
    }
    std::panic::catch_unwind(|| {
        let Some(game) = path(game) else {
            return std::ptr::null_mut();
        };
        let Some(mods) = path(mods) else {
            append_runtime_log(&game, "Runtime initialization failed: null mods path");
            return std::ptr::null_mut();
        };
        match create_runtime(game.clone(), mods) {
            Ok(handle) => Box::into_raw(Box::new(handle)),
            Err(error) => {
                append_runtime_log(&game, &format!("Runtime initialization failed: {error}"));
                std::ptr::null_mut()
            }
        }
    })
    .unwrap_or(std::ptr::null_mut())
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
