use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result_with_revision;
use shroudforge_package::{read_mod_config_revision, save_mod_settings};
use std::time::Instant;

pub(crate) fn apply_mod_settings(
    id: String,
    values: serde_json::Value,
    revision: String,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    let result = save_mod_settings(&arguments.root, &id, &values, &revision);
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
        "Save mod settings",
        result,
        "settings.saved",
        None,
        false,
        saved.as_deref(),
    );
    *next_refresh = Instant::now();
}
