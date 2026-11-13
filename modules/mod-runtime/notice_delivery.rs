use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
/// Owner-thread producer only. The feed remains authoritative when rendering is absent.
pub(crate) fn publish(
    root: &Path,
    package: &str,
    id: &str,
    title: &str,
    message: &str,
    level: u32,
) -> Result<(), String> {
    if !crate::notice_protocol::valid(package, id, title, message, level) {
        return Err("invalid notice payload".into());
    }
    let mirror_level = crate::notice_protocol::level_name(level).unwrap();
    let feed_level = crate::notice_protocol::feed_level_name(level).unwrap();
    let namespaced = format!("{package}.{id}");
    let payload = serde_json::json!({"id":&namespaced,"modId":package,"title":title,"message":message,
        "level":feed_level,"actionUrl":null,"updatedAt":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()});
    crate::notice_protocol::persist_then_mirror(
        || shroudforge_news::write_event(root, &format!("{package}-{id}"), &payload),
        || {
            shroudforge_runtime_bridge::publish_notification(
                &namespaced,
                title,
                message,
                mirror_level,
            )
        },
    )
}
