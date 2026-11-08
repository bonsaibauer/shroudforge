use serde_json::Value;
use std::path::Path;

pub fn migrate_wiki_refresh_interval(root: &Path) -> Result<bool, String> {
    let _lock = crate::config::installation_lock(root)?;
    let path = crate::config::document_path(root, crate::documents::LOADER);
    let mut settings = crate::config::read_loader_unvalidated(root)?;
    let Some(wiki) = settings
        .pointer_mut("/modules/wiki")
        .and_then(Value::as_object_mut)
    else {
        return Ok(false);
    };
    let Some(minutes) = wiki.get("refreshIntervalMinutes").and_then(Value::as_u64) else {
        return Ok(false);
    };
    let normalized = if minutes == 5 {
        1440
    } else if minutes < 60 {
        60
    } else {
        return Ok(false);
    };
    wiki.insert("refreshIntervalMinutes".into(), Value::from(normalized));
    crate::config::validate_document(root, crate::documents::LOADER, &settings)?;
    crate::config::write_json(&path, &settings)?;
    Ok(true)
}
