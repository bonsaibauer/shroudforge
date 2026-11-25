use crate::validate_event;
use std::path::Path;

/// A mod-authored notice. The caller supplies its authenticated package id
/// separately, never from the notice payload.
pub struct ModNotice<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub message: &'a str,
    pub level: &'a str,
    pub action_url: Option<&'a str>,
}

pub fn valid_mod_notice(package: &str, notice: &ModNotice<'_>) -> bool {
    fn identifier(value: &str, maximum: usize) -> bool {
        !value.is_empty()
            && value.len() <= maximum
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    }
    identifier(package, 256)
        && identifier(notice.id, 80)
        && package.len() + notice.id.len() + 1 <= 200
        && notice.title.len() <= 120
        && notice.message.len() <= 2_000
        && matches!(notice.level, "info" | "success" | "warning" | "error" | "update")
        && notice.action_url.is_none_or(|url| url.starts_with("https://"))
}

/// Persist through the canonical feed before optional in-game presentation.
/// Presentation failure does not change persistence success. The presenter
/// receives the original styling level, the feed its existing severity names.
pub fn publish_mod_notice(
    root: &Path,
    package: &str,
    notice: ModNotice<'_>,
    present: impl FnOnce(&str, &str, &str, &str) -> i32,
) -> Result<(), String> {
    if !valid_mod_notice(package, &notice) {
        return Err("invalid notification payload".into());
    }
    let id = format!("{package}.{}", notice.id);
    let severity = match notice.level {
        "success" | "update" => "info",
        "warning" => "warn",
        level => level,
    };
    let payload = serde_json::json!({
        "id": &id, "modId": package,
        "title": notice.title, "message": notice.message,
        "level": severity, "actionUrl": notice.action_url,
        "updatedAt": crate::now(),
    });
    write_event(root, &format!("{package}-{}", notice.id), &payload)?;
    let _ = present(&id, notice.title, notice.message, notice.level);
    Ok(())
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
