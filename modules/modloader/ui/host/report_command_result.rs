use super::mark_control_action::mark_control_action;
use super::notify_command_result::notify_command_result_with_revision;
use shroudforge_logging::{activity_log_level, write_activity_at_level};
use shroudforge_package::package_lookup::mod_ecosystem;
use std::path::Path;

pub(crate) fn is_module_unavailable_warning(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("disabled in settings")
        || message.contains("is disabled in module settings")
}

pub(crate) fn disable_missing_updater(root: &Path) -> String {
    let executable = shroudforge_loader_config::paths::updater_executable(root);
    let initialization_error = shroudforge_loader_config::config::initialize_loader_config(root)
        .err()
        .map(|error| format!(" Could not save the disabled setting: {error}"))
        .unwrap_or_default();
    let _ = shroudforge_loader_config::config::request_window_visibility(root, "updates", false);
    format!(
        "Updates module is disabled because shroudforge-updater.exe is missing: {}.{initialization_error}",
        executable.display()
    )
}

pub(crate) fn report_command_result(
    root: &Path,
    webview: &wry::WebView,
    request_id: &str,
    source: &str,
    action: &str,
    result: Result<(), String>,
    success_message: &str,
    batch_key: Option<&str>,
    restore_settings: bool,
) {
    report_command_result_with_revision(
        root,
        webview,
        request_id,
        source,
        action,
        result,
        success_message,
        batch_key,
        restore_settings,
        None,
    );
}

/// Same, with the new revision of the saved mod state, so the page can save
/// again at once instead of waiting for its next refresh.
pub(crate) fn report_command_result_with_revision(
    root: &Path,
    webview: &wry::WebView,
    request_id: &str,
    source: &str,
    action: &str,
    result: Result<(), String>,
    success_message: &str,
    batch_key: Option<&str>,
    restore_settings: bool,
    revision: Option<&str>,
) {
    let unavailable = result
        .as_ref()
        .err()
        .is_some_and(|message| is_module_unavailable_warning(message));
    let hotkey_conflict = result.as_ref().err().is_some_and(|message| {
        message.starts_with("Hotkey ") && message.contains(" conflicts between ")
    });
    match &result {
        Ok(()) => mark_control_action(root, request_id, "succeeded", success_message),
        Err(error) if unavailable => mark_control_action(root, request_id, "idle", error),
        Err(error) => mark_control_action(root, request_id, "failed", error),
    }
    let (activity_result, details, outcome) = match &result {
        Ok(()) => ("Succeeded", None, "success"),
        Err(error) if unavailable => ("Unavailable", Some(error.as_str()), "warn"),
        Err(error) if hotkey_conflict => ("Conflict", Some(error.as_str()), "warn"),
        Err(error) => ("Failed", Some(error.as_str()), "error"),
    };
    let log_level = activity_log_level(action, outcome);
    write_activity_at_level(root, source, action, activity_result, details, log_level);
    let ecosystem = mod_ecosystem(root, source);
    let notification_result = if unavailable {
        Err("settings.moduleUnavailable".to_owned())
    } else {
        result
    };
    notify_command_result_with_revision(
        webview,
        request_id,
        notification_result,
        success_message,
        log_level,
        batch_key,
        restore_settings,
        ecosystem,
        revision,
    );
}
