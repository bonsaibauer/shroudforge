//! Shared locations for configured news and installation-specific notification state.
use std::{fs, path::{Path, PathBuf}};

pub fn directory(root: &Path) -> PathBuf { crate::paths::config_dir(root).join("news") }
pub fn events_directory(root: &Path) -> PathBuf { directory(root).join("events") }

pub fn validate_event(root: &Path, value: &serde_json::Value) -> Result<(), String> {
    let path = directory(root).join("event-schema.json");
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => include_str!("../../../config/news/event-schema.json").into(),
        Err(error) => return Err(error.to_string()),
    };
    let schema = serde_json::from_str(&source).map_err(|e| e.to_string())?;
    jsonschema::validator_for(&schema).map_err(|e| e.to_string())?.validate(value).map_err(|e| e.to_string())
}

pub fn write_event(root: &Path, filename: &str, value: &serde_json::Value) -> Result<(), String> {
    if filename.is_empty() || filename.len() > 240 || filename == "." || filename == ".." || !filename.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'.'|b'-'|b'_'|b'+')) { return Err("invalid notification filename".into()); }
    validate_event(root, value)?;
    crate::config::update_state_section(root,"events",|stored| {
        let mut events=stored.cloned().unwrap_or_else(||serde_json::json!({}));
        events[format!("{filename}.json")]=value.clone();
        Ok(events)
    })
}

fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

fn read_state(root: &Path) -> Result<serde_json::Value, String> {
    let mut value: serde_json::Value = match crate::config::read_document(root, "news-state") {
        Ok(value) => value,
        Err(error) if error.contains("has no news state") => serde_json::json!({"read": [], "readAt": {}}),
        Err(error) => return Err(error),
    };
    if !value.is_object() { return Err("news read state must be an object".into()); }
    if value.get("readAt").is_none() { value["readAt"] = serde_json::json!({}); }
    if !value["readAt"].is_object() { return Err("news readAt must be an object".into()); }
    let ids = value.get("read").and_then(|v| v.as_array()).ok_or("news read state must contain a read array")?.clone();
    for id in ids {
        let id = id.as_str().ok_or("news read id must be a string")?;
        if value["readAt"].get(id).is_none() {
            value["readAt"][id] = serde_json::json!(0);
        }
    }
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
    Ok(state["read"].as_array().unwrap().iter().filter_map(|id| {
        let id = id.as_str()?;
        let interval = messages.iter().find(|message| message["id"] == id)
            .and_then(|message| message["repeatEveryDays"].as_u64());
        if let Some(days) = interval {
            let timestamp = state["readAt"][id].as_u64().unwrap_or(0);
            if now().saturating_sub(timestamp) >= days.saturating_mul(86400) { return None; }
        }
        Some(id.to_owned())
    }).collect())
}

pub fn mark_read(root: &Path, ids: &[String]) -> Result<(), String> {
    crate::config::update_state_section(root, "news", |stored| {
        let mut value = stored.cloned().unwrap_or(serde_json::json!({"read": [], "readAt": {}}));
        for id in ids {
            if !value["read"].as_array().is_some_and(|read| read.iter().any(|old| old == id)) { value["read"].as_array_mut().ok_or("news read must be an array")?.push(serde_json::json!(id)); }
            value["readAt"][id] = serde_json::json!(now());
        }
        validate_state(root, &value)?;
        Ok(value)
    })
}

/// Copy legacy state once, preserving the original files as a recovery copy.
/// Existing destination files always win, including events published during migration.
pub fn migrate(root: &Path) -> Result<(), String> {
    crate::config::migrate_state(root)?;
    let legacy = crate::paths::ui_data_dir(root).join("notifications");
    if legacy.is_dir() {
        for entry in fs::read_dir(legacy).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().is_some_and(|extension| extension == "json") {
                let bytes=fs::read(entry.path()).map_err(|e|e.to_string())?;
                let value:serde_json::Value=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
                let filename=entry.file_name().to_string_lossy().trim_end_matches(".json").to_owned();
                validate_event(root,&value)?;
                crate::config::update_state_section(root,"events",|stored| {
                    let mut events=stored.cloned().unwrap_or_else(||serde_json::json!({}));
                    events.as_object_mut().ok_or("notification state must be an object")?.entry(format!("{filename}.json")).or_insert(value.clone());
                    Ok(events)
                })?;
            }
        }
    }
    Ok(())
}

pub fn read(root: &Path) -> Result<serde_json::Value, String> {
    let value: serde_json::Value = serde_json::from_str(include_str!("../../../config/news/news.json"))
        .map_err(|e| format!("embedded news.json: {e}"))?;
    crate::config::validate_document(root, "news", &value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurring_news_becomes_unread_and_can_be_acknowledged_again() {
        let root=tempfile::tempdir().unwrap(); let root=root.path();
        crate::config::write_document(root,"news-state",&serde_json::json!({
            "read":["shroudforge-support-1","shroudforge-welcome-1.0.0"],
            "readAt":{"shroudforge-support-1":now()-91*86400,"shroudforge-welcome-1.0.0":now()-91*86400}
        })).unwrap();
        assert_eq!(read_ids(root).unwrap(),vec!["shroudforge-welcome-1.0.0"]);
        mark_read(root,&["shroudforge-support-1".into()]).unwrap();
        assert_eq!(read_ids(root).unwrap(),vec!["shroudforge-support-1","shroudforge-welcome-1.0.0"]);
    }

    #[test]
    fn invalid_state_is_reported_and_preserved() {
        let root=tempfile::tempdir().unwrap(); let root=root.path();
        crate::config::write_json(&crate::paths::config_dir(root).join("state.json"),&serde_json::json!({"schemaVersion":1,"news":{"read":[],"readAt":false}})).unwrap();
        assert!(read_ids(root).is_err());
    }

    #[test]
    fn migration_preserves_read_state_and_newer_events() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let old = crate::paths::ui_data_dir(root).join("notifications/event-one.json");
        crate::config::write_json(&old, &serde_json::json!({"id":"one","message":"old","title":"T","level":"info"})).unwrap();
        crate::config::write_json(&crate::paths::ui_data_dir(root).join("news-state.json"), &serde_json::json!({"read":["one"]})).unwrap();
        let new = serde_json::json!({"id":"one","message":"new","title":"T","level":"info"});
        write_event(root,"event-one",&new).unwrap();
        migrate(root).unwrap();
        assert!(old.is_file());
        let value = crate::config::read_document(root,"events-state").unwrap();
        assert_eq!(value["event-one.json"]["message"], "new");
        let value = crate::config::read_document(root,"news-state").unwrap();
        assert_eq!(value["read"][0], "one");
        crate::config::write_document(root,"news-state", &serde_json::json!({"read":[],"readAt":{}})).unwrap();
        migrate(root).unwrap();
        let value = crate::config::read_document(root,"news-state").unwrap();
        assert_eq!(value["read"], serde_json::json!([]));
    }
}
