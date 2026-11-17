use super::read_activity_log::{read_activity, selected_server_target};
use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result;
use std::time::Instant;

pub(crate) fn refresh_activity_log(
    request_id: Option<String>,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    if let Some(request_id) = request_id {
        let result = read_activity(&arguments.root, selected_server_target(&arguments.root)).map(|_| ());
        report_command_result(
            &arguments.root,
            &webview,
            &request_id,
            "ShroudForge Log",
            "Refresh log",
            result,
            "activity.refreshed",
            None,
            false,
        );
    }
    *next_refresh = Instant::now();
}
