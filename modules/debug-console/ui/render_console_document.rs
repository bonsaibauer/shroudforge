pub(crate) fn render_console_document() -> String {
    let styles = format!(
        "{}\n{}",
        include_str!("../../ui-resources/tokens.css"),
        include_str!("styles.css")
    );
    let hammer_icon = png_data_uri(include_bytes!("../../../assets/shroudedit-hammer.png"));
    let html = include_str!("index.html")
        .replace("__SHROUDFORGE_STYLES__", &styles)
        .replace("__SHROUDFORGE_HAMMER_ICON__", &hammer_icon)
        .replace("__SHROUDFORGE_SCRIPT__", include_str!("app.ts"));
    html
}

fn png_data_uri(png: &[u8]) -> String {
    const BASE64: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity((png.len() + 2) / 3 * 4);
    for chunk in png.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let block = (first << 16) | (second << 8) | third;
        encoded.push(BASE64[((block >> 18) & 0x3f) as usize] as char);
        encoded.push(BASE64[((block >> 12) & 0x3f) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            BASE64[((block >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            BASE64[(block & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    format!("data:image/png;base64,{encoded}")
}
