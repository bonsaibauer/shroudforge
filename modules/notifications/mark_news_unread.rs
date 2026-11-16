use crate::valid_news_id;
use std::path::Path;

pub fn mark_unread(root: &Path, ids: &[String]) -> Result<(), String> {
    mark_unread_for_target(root, ids, false)
}

pub fn mark_unread_for_target(root: &Path, ids: &[String], server: bool) -> Result<(), String> {
    crate::config::update_state_section(root, "news", |stored| {
        let mut value = stored
            .cloned()
            .unwrap_or(serde_json::json!({"read": [], "readAt": {}}));
        let (read_key, read_at_key) = if server {
            if value["serverRead"].is_null() {
                value["serverRead"] = serde_json::json!([]);
            }
            if value["serverReadAt"].is_null() {
                value["serverReadAt"] = serde_json::json!({});
            }
            ("serverRead", "serverReadAt")
        } else {
            ("read", "readAt")
        };
        let requested: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
        let read = value[read_key]
            .as_array_mut()
            .ok_or("news read must be an array")?;
        read.retain(|id| id.as_str().map_or(true, |id| !requested.contains(id)));
        if let Some(timestamps) = value[read_at_key].as_object_mut() {
            timestamps.retain(|id, _| !requested.contains(id.as_str()));
        }
        crate::config::validate_document(root, crate::documents::NEWS_STATE, &value)?;
        Ok(value)
    })
}

pub fn mark_news_unread(root: &Path, ids: &[String]) -> Result<(), String> {
    mark_news_unread_for_target(root, ids, false)
}

pub fn mark_news_unread_for_target(
    root: &Path,
    ids: &[String],
    server: bool,
) -> Result<(), String> {
    let ids = ids
        .iter()
        .filter(|id| valid_news_id(id))
        .cloned()
        .collect::<Vec<_>>();
    mark_unread_for_target(root, &ids, server)
}
