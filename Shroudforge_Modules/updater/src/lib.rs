#[path = "main.rs"]
mod application;

pub use application::run_module;
pub use application::{request_install_after_game, request_mod_install, request_system_stage, request_worker};
