use super::notify_command_result::notify_command_result;
use serde::{Deserialize, Serialize};
use shroudforge_logging::{LogLevel, write_activity_at_level};
use std::path::Path;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UiLogEvent {
    level: UiLogLevel,
    source: UiLogSource,
    operation: String,
    subject: String,
    message: String,
    #[serde(default)]
    notify: bool,
    #[serde(default, rename = "notifyMessage")]
    notify_message: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
enum UiLogLevel {
    Info,
    Warn,
    Error,
    Debug,
    Trace,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum UiLogSource {
    Wiki,
    Compatibility,
    Modloader,
}

pub(crate) fn record_ui_log(root: &Path, webview: &wry::WebView, event: UiLogEvent) {
    if !valid_text(&event.operation, 120)
        || !valid_text(&event.subject, 120)
        || !valid_text(&event.message, 2000)
    {
        let _ = shroudforge_logging::append(
            root,
            'W',
            "modloader-ui",
            "UI log event was ignored because its text was empty, too long or contained control characters.",
        );
        return;
    }
    let level = match event.level {
        UiLogLevel::Info => LogLevel::Info,
        UiLogLevel::Warn => LogLevel::Warn,
        UiLogLevel::Error => LogLevel::Error,
        UiLogLevel::Debug => LogLevel::Debug,
        UiLogLevel::Trace => LogLevel::Trace,
    };
    let source = match event.source {
        UiLogSource::Wiki => "wiki",
        UiLogSource::Compatibility => "compatibility",
        UiLogSource::Modloader => "modloader",
    };
    write_activity_at_level(
        root,
        source,
        &event.operation,
        &event.subject,
        Some(&event.message),
        level,
    );
    if event.notify && matches!(level, LogLevel::Warn | LogLevel::Error) {
        let notification = event
            .notify_message
            .as_deref()
            .filter(|message| valid_text(message, 2000))
            .unwrap_or(&event.message);
        notify_command_result(
            webview,
            "",
            Err(notification.to_owned()),
            "",
            level,
            None,
            false,
            None,
        );
    }
}

fn valid_text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= limit
        && !value.chars().any(char::is_control)
}
