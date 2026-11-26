use super::read_editor_arguments::Arguments;
use crate::log_editor_action::log_world_editor;
use crate::queue_editor_action::dispatch_action;
use std::time::Instant;

pub(crate) fn run_blueprint_action(
    action: String,
    value: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    let result = dispatch_action(&arguments.root, &action, &value);
    match result {
        Ok(_) => *next_refresh = Instant::now(),
        Err(error) => {
            log_world_editor(
                &arguments.root,
                'E',
                "aktion",
                &format!("Action \"{action}\" failed: {error}"),
            );
        }
    }
}
