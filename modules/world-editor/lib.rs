#[cfg(windows)]
#[path = "ui/mod.rs"]
mod host;

#[cfg(windows)]
pub fn run_module() -> Result<(), Box<dyn std::error::Error>> {
    host::window::run_module()
}

#[cfg(not(windows))]
pub fn run_module() -> Result<(), Box<dyn std::error::Error>> {
    Err("World Editor UI is supported on Windows only".into())
}

#[cfg(windows)]
mod blueprint_library;
#[cfg(windows)]
mod log_editor_action;
#[cfg(windows)]
mod panel_layout;
#[cfg(windows)]
mod queue_editor_action;
