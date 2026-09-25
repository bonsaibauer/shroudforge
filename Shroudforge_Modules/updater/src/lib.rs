#[path = "main.rs"]
mod application;

pub use application::run_module;
pub use application::{request_install_after_game, request_mod_install, request_mod_update, request_runtime_mod_reload, request_runtime_mod_unload, request_system_stage, request_update_window, request_worker};
