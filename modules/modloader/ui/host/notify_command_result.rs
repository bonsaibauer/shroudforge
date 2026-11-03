use shroudforge_logging::LogLevel;

pub(crate) fn notify_command_result(
    webview: &wry::WebView,
    request_id: &str,
    result: Result<(), String>,
    success_message: &str,
    level: LogLevel,
    batch_key: Option<&str>,
    restore_settings: bool,
    ecosystem: Option<&str>,
) {
    notify_command_result_with_revision(
        webview,
        request_id,
        result,
        success_message,
        level,
        batch_key,
        restore_settings,
        ecosystem,
        None,
    );
}

/// Same, with the new revision of a saved mod state (see report_command_result_with_revision).
pub(crate) fn notify_command_result_with_revision(
    webview: &wry::WebView,
    request_id: &str,
    result: Result<(), String>,
    success_message: &str,
    level: LogLevel,
    batch_key: Option<&str>,
    restore_settings: bool,
    ecosystem: Option<&str>,
    revision: Option<&str>,
) {
    let (success, message) = match result {
        Ok(()) => (true, success_message.to_owned()),
        Err(error) => (false, error),
    };
    let payload = serde_json::json!({"requestId":request_id,"success":success,"message":message,"level":level.as_str(),"batchKey":batch_key,"restoreSettings":restore_settings,"ecosystem":ecosystem,"revision":revision});
    if let Ok(payload) = serde_json::to_string(&payload) {
        let _ = webview.evaluate_script(&format!("window.__shroudforgeCommandResult({payload});"));
    }
}
