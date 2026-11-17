use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result;
use std::time::Instant;

pub(crate) fn change_notice_read_state(
    ids: Vec<String>,
    read: bool,
    server: bool,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    next_refresh: &mut Instant,
) {
    let result = if read {
        shroudforge_news::mark_news_read_for_target(&arguments.root, &ids, server)
    } else {
        shroudforge_news::mark_news_unread_for_target(&arguments.root, &ids, server)
    };
    report_command_result(
        &arguments.root,
        &webview,
        &request_id,
        "Modloader",
        if read {
            "Mark notices as read"
        } else {
            "Mark notices as unread"
        },
        result,
        if read {
            "news.markedReadToast"
        } else {
            "news.markedUnreadToast"
        },
        None,
        false,
    );
    *next_refresh = Instant::now();
}
