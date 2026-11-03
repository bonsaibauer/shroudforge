use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result_with_revision;
use shroudforge_package::{read_mod_config_revision, set_mod_enabled};
use std::time::Instant;

pub(crate) fn apply_mod_enabled(
    id: String,
    enabled: bool,
    revision: String,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    let result = set_mod_enabled(&arguments.root, &id, enabled, &revision);
    let action = if enabled { "Enable mod" } else { "Disable mod" };
    let message = if enabled {
        "mod.enabledToast"
    } else {
        "mod.disabledToast"
    };
    // The page saves the next change against this revision.
    let saved = result
        .is_ok()
        .then(|| read_mod_config_revision(&arguments.root, &id).ok())
        .flatten();
    report_command_result_with_revision(
        &arguments.root,
        &webview,
        &request_id,
        &id,
        action,
        result,
        message,
        None,
        false,
        saved.as_deref(),
    );
    *next_refresh = Instant::now();
}
