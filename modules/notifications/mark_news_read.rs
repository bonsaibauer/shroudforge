use crate::now;
use crate::valid_news_id;
use std::path::Path;

pub fn mark_read(root: &Path, ids: &[String]) -> Result<(), String> {
    mark_read_for_target(root, ids, false)
}

pub fn mark_read_for_target(root: &Path, ids: &[String], server: bool) -> Result<(), String> {
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
        for id in ids {
            if !value[read_key]
                .as_array()
                .is_some_and(|read| read.iter().any(|old| old == id))
            {
                value[read_key]
                    .as_array_mut()
                    .ok_or("news read must be an array")?
                    .push(serde_json::json!(id));
            }
            value[read_at_key][id] = serde_json::json!(now());
        }
        crate::config::validate_document(root, crate::documents::NEWS_STATE, &value)?;
        Ok(value)
    })
}

pub fn mark_news_read(root: &Path, ids: &[String]) -> Result<(), String> {
    mark_news_read_for_target(root, ids, false)
}

pub fn mark_news_read_for_target(
    root: &Path,
    ids: &[String],
    server: bool,
) -> Result<(), String> {
    let ids = ids
        .iter()
        .filter(|id| valid_news_id(id))
        .cloned()
        .collect::<Vec<_>>();
    mark_read_for_target(root, &ids, server)
}
