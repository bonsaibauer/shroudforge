//! Shared locations for configured news and installation-specific notification state.
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn directory(root: &Path) -> PathBuf {
    crate::paths::config_dir(root).join("news")
}
pub fn events_directory(root: &Path) -> PathBuf {
    directory(root).join("events")
}

pub fn validate_event(root: &Path, value: &serde_json::Value) -> Result<(), String> {
    let path = directory(root).join("event-schema.json");
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            include_str!("news/event-schema.json").into()
        }
        Err(error) => return Err(error.to_string()),
    };
    let schema = serde_json::from_str(&source).map_err(|e| e.to_string())?;
    jsonschema::validator_for(&schema)
        .map_err(|e| e.to_string())?
        .validate(value)
        .map_err(|e| e.to_string())
}

pub fn write_event(root: &Path, filename: &str, value: &serde_json::Value) -> Result<(), String> {
    if filename.is_empty()
        || filename.len() > 240
        || filename == "."
        || filename == ".."
        || !filename
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
    {
        return Err("invalid notification filename".into());
    }
    validate_event(root, value)?;
    crate::config::update_state_section(root, "events", |stored| {
        let mut events = stored.cloned().unwrap_or_else(|| serde_json::json!({}));
        events[format!("{filename}.json")] = value.clone();
        Ok(events)
    })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_state(root: &Path) -> Result<serde_json::Value, String> {
    let value: serde_json::Value = match crate::config::read_document(root, "news-state") {
        Ok(value) => value,
        Err(error) if error.contains("has no news state") => {
            serde_json::json!({"read": [], "readAt": {}})
        }
        Err(error) => return Err(error),
    };
    validate_state(root, &value)?;
    Ok(value)
}

fn validate_state(root: &Path, value: &serde_json::Value) -> Result<(), String> {
    crate::config::validate_document(root, "news-state", value)
}

pub fn read_ids(root: &Path) -> Result<Vec<String>, String> {
    let state = read_state(root)?;
    let news = read(root)?;
    let messages = news["messages"].as_array().ok_or("missing news messages")?;
    let mut unread = Vec::new();
    for value in state["read"].as_array().unwrap() {
        let id = value.as_str().ok_or("news read id must be a string")?;
        let timestamp = state["readAt"][id]
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

pub fn mark_read(root: &Path, ids: &[String]) -> Result<(), String> {
    crate::config::update_state_section(root, "news", |stored| {
        let mut value = stored
            .cloned()
            .unwrap_or(serde_json::json!({"read": [], "readAt": {}}));
        for id in ids {
            if !value["read"]
                .as_array()
                .is_some_and(|read| read.iter().any(|old| old == id))
            {
                value["read"]
                    .as_array_mut()
                    .ok_or("news read must be an array")?
                    .push(serde_json::json!(id));
            }
            value["readAt"][id] = serde_json::json!(now());
        }
        validate_state(root, &value)?;
        Ok(value)
    })
}

pub fn mark_unread(root: &Path, ids: &[String]) -> Result<(), String> {
    crate::config::update_state_section(root, "news", |stored| {
        let mut value = stored
            .cloned()
            .unwrap_or(serde_json::json!({"read": [], "readAt": {}}));
        let requested: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
        let read = value["read"]
            .as_array_mut()
            .ok_or("news read must be an array")?;
        read.retain(|id| id.as_str().map_or(true, |id| !requested.contains(id)));
        if let Some(timestamps) = value["readAt"].as_object_mut() {
            timestamps.retain(|id, _| !requested.contains(id.as_str()));
        }
        validate_state(root, &value)?;
        Ok(value)
    })
}

pub fn read(root: &Path) -> Result<serde_json::Value, String> {
    let value: serde_json::Value = serde_json::from_str(include_str!("news/news.json"))
        .map_err(|e| format!("embedded news.json: {e}"))?;
    crate::config::validate_document(root, "news", &value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurring_news_becomes_unread_and_can_be_acknowledged_again() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        crate::config::write_document(root,"news-state",&serde_json::json!({
            "read":["shroudforge-support-2","shroudforge-welcome-1.6.5"],
            "readAt":{"shroudforge-support-2":now()-91*86400,"shroudforge-welcome-1.6.5":now()-91*86400}
        })).unwrap();
        assert_eq!(read_ids(root).unwrap(), vec!["shroudforge-welcome-1.6.5"]);
        mark_read(root, &["shroudforge-support-2".into()]).unwrap();
        assert_eq!(
            read_ids(root).unwrap(),
            vec!["shroudforge-support-2", "shroudforge-welcome-1.6.5"]
        );
    }

    #[test]
    fn invalid_state_is_reported_and_preserved() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        crate::config::write_json(
            &crate::paths::state_file(root),
            &serde_json::json!({"schemaVersion":1,"news":{"read":[],"readAt":false}}),
        )
        .unwrap();
        assert!(read_ids(root).is_err());
    }
}
