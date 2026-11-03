use std::borrow::Cow;
use wry::WebViewBuilder;
use wry::WebViewBuilderExtWindows;

pub fn with_embedded_document<'a>(
    builder: WebViewBuilder<'a>,
    html: impl Into<Cow<'static, [u8]>> + 'static,
) -> WebViewBuilder<'a> {
    let html = html.into();
    // WebView2 caps NavigateToString at 2 MiB. Serve embedded pages through a URL instead.
    builder
        .with_custom_protocol("shroudforge".into(), move |_webview_id, request| {
            let (status, content_type, body) = if request.uri().path() == "/index.html" {
                (
                    wry::http::StatusCode::OK,
                    "text/html; charset=utf-8",
                    html.clone(),
                )
            } else {
                (
                    wry::http::StatusCode::NOT_FOUND,
                    "text/plain; charset=utf-8",
                    Cow::Borrowed(&[][..]),
                )
            };
            wry::http::Response::builder()
                .status(status)
                .header(wry::http::header::CONTENT_TYPE, content_type)
                .body(body)
                .expect("embedded ShroudForge UI response is valid")
        })
        .with_https_scheme(true)
        .with_url("shroudforge://localhost/index.html")
}
