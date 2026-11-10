use super::notify_command_result::notify_command_result;
use super::open_external_link::open_url;
use super::read_host_arguments::Arguments;
use shroudforge_logging::{activity_log_level, write_activity, write_activity_at_level};

pub(crate) fn open_requested_link(
    url: String,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
) {
    match open_url(&url) {
        Ok(()) => {
            write_activity(
                &arguments.root,
                "modloader-ui",
                "Open external link",
                "Succeeded",
                None,
                "info",
            );
            if url.trim_end_matches('/') == "https://wiki.shroudforge.net" {
                notify_command_result(
                    webview,
                    &request_id,
                    Ok(()),
                    "wiki.openSucceeded",
                    shroudforge_logging::LogLevel::Info,
                    None,
                    false,
                    None,
                );
            }
        }
        Err(error) => {
            let level = activity_log_level("Open external link", "error");
            write_activity_at_level(
                &arguments.root,
                "modloader-ui",
                "Open external link",
                "Failed",
                Some(&error),
                level,
            );
            notify_command_result(
                &webview,
                &request_id,
                Err(error),
                "",
                level,
                None,
                false,
                None,
            );
        }
    }
}
