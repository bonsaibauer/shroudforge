use crate::*;
pub fn request_mod_install(
    root: &std::path::Path,
    project_id: &str,
    title: &str,
    provider: serde_json::Value,
    version_id: Option<&str>,
    content_type: &str,
) -> Result<(), String> {
    if !crate::update_check::updates_enabled(root) {
        return Err(crate::UPDATE_MODULE_DISABLED_MESSAGE.into());
    }
    if !matches!(content_type, "mod" | "schematic" | "modpack") {
        return Err("unsupported ShroudEdit content type for installation".into());
    }
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("invalid mod project ID".into());
    }
    validate_version_id(version_id)?;
    enqueue_update_item(
        root,
        serde_json::json!({"id":format!("{content_type}-install:{project_id}"),"kind":"mod","operation":"install","title":title,"version":"","state":"queued","selected":true,"payload":{"projectId":project_id,"versionId":version_id,"provider":provider,"contentType":content_type}}),
    )
    .map(|_| ())
}

pub fn request_mod_update(
    root: &std::path::Path,
    project_id: &str,
    mod_id: &str,
    title: &str,
    provider: serde_json::Value,
    version_id: Option<&str>,
) -> Result<(), String> {
    validate_version_id(version_id)?;
    enqueue_mod_update(root, project_id, mod_id, title, provider, version_id).map(|_| ())
}

fn validate_version_id(version_id: Option<&str>) -> Result<(), String> {
    if version_id.is_some_and(|version_id| {
        version_id.is_empty()
            || version_id.len() > 128
            || !version_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    }) {
        return Err("invalid mod version ID".into());
    }
    Ok(())
}

pub(crate) fn request_mod_update_if_needed(
    root: &std::path::Path,
    project_id: &str,
    mod_id: &str,
    title: &str,
    provider: serde_json::Value,
    version_id: &str,
) -> Result<bool, String> {
    validate_version_id(Some(version_id))?;
    enqueue_mod_update(root, project_id, mod_id, title, provider, Some(version_id))
}

fn enqueue_mod_update(
    root: &std::path::Path,
    project_id: &str,
    mod_id: &str,
    title: &str,
    provider: serde_json::Value,
    version_id: Option<&str>,
) -> Result<bool, String> {
    if !crate::update_check::updates_enabled(root) {
        return Err(crate::UPDATE_MODULE_DISABLED_MESSAGE.into());
    }
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        || mod_id.is_empty()
        || mod_id.len() > 64
        || !mod_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("invalid mod project or package ID".into());
    }
    enqueue_update_item(
        root,
        serde_json::json!({"id":format!("mod-update:{project_id}:{mod_id}"),"kind":"mod","operation":"update","title":title,"version":"","state":"queued","selected":true,"payload":{"projectId":project_id,"modId":mod_id,"versionId":version_id,"provider":provider}}),
    )
}
