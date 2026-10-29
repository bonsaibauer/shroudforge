use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result;
use shroudforge_package::save_key_reservation;
use std::time::Instant;

/// Saves a padlock the player turned in front of a mod keybind.
pub(crate) fn apply_key_reservation(
    id: String,
    key: String,
    device: shroudforge_hotkeys::InputDevice,
    reserved: bool,
    revision: String,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    let result = save_key_reservation(&arguments.root, &id, &key, device, reserved, &revision);
    report_command_result(
        &arguments.root,
        &webview,
        &request_id,
        &id,
        "Save key reservation",
        result,
        "settings.saved",
        None,
        false,
    );
    *next_refresh = Instant::now();
}
