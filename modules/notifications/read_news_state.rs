use crate::{now, read};
use std::path::Path;

fn read_state(root: &Path) -> Result<serde_json::Value, String> {
    let value: serde_json::Value =
        match crate::config::read_document(root, crate::documents::NEWS_STATE) {
            Ok(value) => value,
            Err(error) if error.contains("has no news state") => {
                serde_json::json!({"read": [], "readAt": {}})
            }
            Err(error) => return Err(error),
        };
    crate::config::validate_document(root, crate::documents::NEWS_STATE, &value)?;
    Ok(value)
}

pub fn read_ids(root: &Path) -> Result<Vec<String>, String> {
    read_ids_for_target(root, false)
}

pub fn read_server_ids(root: &Path) -> Result<Vec<String>, String> {
    read_ids_for_target(root, true)
}

fn read_ids_for_target(root: &Path, server: bool) -> Result<Vec<String>, String> {
    let state = read_state(root)?;
    let news = read(root)?;
    let messages = news["messages"].as_array().ok_or("missing news messages")?;
    let mut unread = Vec::new();
    let (read_key, read_at_key) = if server {
        ("serverRead", "serverReadAt")
    } else {
        ("read", "readAt")
    };
    for value in state[read_key].as_array().into_iter().flatten() {
        let id = value.as_str().ok_or("news read id must be a string")?;
        let timestamp = state[read_at_key][id]
            .as_u64()
            .ok_or_else(|| format!("news read ID has no timestamp: {id}"))?;
        let interval = messages
            .iter()
            .find(|message| message["id"] == id)
            .and_then(|message| message["repeatEveryDays"].as_u64());
        if let Some(days) = interval {
            if now().saturating_sub(timestamp) >= days.saturating_mul(86400) {
                continue;
            }
        }
        unread.push(id.to_owned());
    }
    Ok(unread)
}

pub fn read_news_state(root: &Path) -> Vec<String> {
    crate::read_ids(root).unwrap_or_else(|error| {
        eprintln!("news state: {error}");
        Vec::new()
    })
}
