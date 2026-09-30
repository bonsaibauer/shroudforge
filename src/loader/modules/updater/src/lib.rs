#[path = "main.rs"]
mod application;

pub use application::request_gamefiles_restore;
pub use application::run_module;
pub use application::{
    cancel_system_update, cancel_update_queue, check_update_cancelled, clear_update_queue,
    enqueue_system_update, read_update_queue, remove_update_queue_item, request_install_after_game,
    request_mod_install, request_mod_update, request_runtime_mod_reload,
    request_runtime_mod_unload, request_system_stage, request_worker, select_update_queue_item,
    select_update_queue_items, set_active_queue_item_state, start_update_queue,
};
