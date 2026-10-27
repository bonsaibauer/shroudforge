use super::read_host_arguments::Arguments;
use super::read_installed_mods::read_mods;
use super::report_command_result::report_command_result;
use std::time::Instant;

pub(crate) fn refresh_installed_mod(
    id: String,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    let config = super::read_ui_configuration::read_central_config(&arguments.root);
    let locale = config
        .pointer("/general/locale")
        .and_then(|value| value.as_str())
        .filter(|locale| super::save_language::is_valid_locale(locale))
        .unwrap_or("en-GB");
    let result = if read_mods(&arguments.root, locale)
        .iter()
        .any(|mod_info| mod_info.id == id)
    {
        Ok(())
    } else {
        Err(format!("Mod '{id}' is no longer installed"))
    };
    report_command_result(
        &arguments.root,
        &webview,
        &request_id,
        &id,
        "Refresh mod",
        result,
        "mod.refreshed",
        None,
        false,
    );
    *next_refresh = Instant::now();
}
