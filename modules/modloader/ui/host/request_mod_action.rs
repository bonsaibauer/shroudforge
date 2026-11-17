use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result;
use shroudforge_package::queue_mod_action;
use std::time::Instant;

pub(crate) fn request_mod_action(
    id: String,
    action: String,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    let result = queue_mod_action(&arguments.root, &id, &action);
    report_command_result(
        &arguments.root,
        &webview,
        &request_id,
        &id,
        "Queue mod action",
        result,
        "mod.actionQueued",
        None,
        false,
    );
    *next_refresh = Instant::now();
}
