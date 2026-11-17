use super::read_host_arguments::Arguments;
use super::read_activity_log::{activity_log_path, selected_server_target};
use shroudforge_logging::write_activity;
use std::fs;

pub(crate) fn copy_activity_log(request_id: String, arguments: &Arguments, webview: &wry::WebView) {
    let log_path = activity_log_path(
        &arguments.root,
        selected_server_target(&arguments.root),
    );
    let result = fs::read_to_string(log_path);
    match &result {
        Ok(_) => write_activity(
            &arguments.root,
            "ShroudForge Log",
            "Copy current log",
            "Succeeded",
            None,
            "success",
        ),
        Err(error) => {
            let detail = error.to_string();
            write_activity(
                &arguments.root,
                "ShroudForge Log",
                "Copy current log",
                "Failed",
                Some(&detail),
                "error",
            );
        }
    }
    let payload = match result {
        Ok(text) => serde_json::json!({
            "requestId": request_id,
            "success": true,
            "text": text,
        }),
        Err(error) => serde_json::json!({
            "requestId": request_id,
            "success": false,
            "error": error.to_string(),
        }),
    };
    if let Ok(payload) = serde_json::to_string(&payload) {
        let _ =
            webview.evaluate_script(&format!("window.__shroudforgeLogCopyResult?.({payload});"));
    }
}
