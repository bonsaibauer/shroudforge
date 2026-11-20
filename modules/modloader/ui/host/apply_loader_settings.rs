use super::loader_settings_input::UiSettings;
use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result;
use super::save_loader_settings::save_settings;
use std::time::Instant;

pub(crate) fn apply_loader_settings(
    settings: UiSettings,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    active_provider: &mut serde_json::Value,
    next_refresh: &mut Instant,
) {
    let remote_target = shroudforge_loader_config::config::read_loader(&arguments.root)
        .ok()
        .and_then(|config| {
            config
                .pointer("/modules/modloaderUi/remoteTarget")
                .and_then(serde_json::Value::as_str)
                .map(|target| target == "server")
        })
        .unwrap_or(false);
    let result = if remote_target && !arguments.server {
        Err("Bulk settings save is still local-only. Save individual server module settings instead.".into())
    } else {
        save_settings(&arguments.root, &settings)
    };
    *next_refresh = Instant::now();
    if result.is_ok() {
        active_provider["baseUrl"] = settings.base_url.into();
        active_provider["projectId"] = settings.project_id.into();
        *next_refresh = Instant::now();
    }
    report_command_result(
        &arguments.root,
        &webview,
        &request_id,
        "Modloader",
        "Save settings",
        result,
        "settings.saved",
        None,
        true,
    );
}
