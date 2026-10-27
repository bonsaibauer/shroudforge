use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result;
use shroudforge_package::HotkeyAssessmentTarget;

pub(crate) fn assess_hotkey_change(
    target: HotkeyAssessmentTarget,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
) {
    let result = shroudforge_package::assess_hotkey_change(&arguments.root, &target);
    let (payload, error) = match result {
        Ok(shroudforge_package::HotkeyAssessment::Allowed) => (
            serde_json::json!({ "requestId": request_id, "allowed": true }),
            None,
        ),
        Ok(shroudforge_package::HotkeyAssessment::Confirm(key)) => (
            serde_json::json!({
                "requestId": request_id,
                "allowed": true,
                "confirmation": { "scancode": key.scancode, "label": key.label },
            }),
            None,
        ),
        Ok(shroudforge_package::HotkeyAssessment::Cancel) => (
            serde_json::json!({
                "requestId": request_id,
                "allowed": false,
                "cancelled": true,
            }),
            None,
        ),
        Err(error) => (
            serde_json::json!({ "requestId": request_id, "allowed": false }),
            Some(error),
        ),
    };
    if let Ok(payload) = serde_json::to_string(&payload) {
        if let Err(error) = webview.evaluate_script(&format!(
            "window.dispatchEvent(new CustomEvent('shroudforge-hotkey-assessed',{{detail:{payload}}}));"
        )) {
            let _ = shroudforge_logging::append(
                &arguments.root,
                'E',
                "hotkeys",
                &format!("Could not return shortcut assessment: {error}"),
            );
        }
    }
    if let Some(error) = error {
        report_command_result(
            &arguments.root,
            webview,
            &request_id,
            "Hotkeys",
            "Assess shortcut assignment",
            Err(error),
            "settings.saved",
            None,
            false,
        );
    }
}
