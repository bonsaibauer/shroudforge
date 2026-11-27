use crate::*;
pub fn request_mod_install(
    root: &std::path::Path,
    project_id: &str,
    title: &str,
    provider: serde_json::Value,
    version_id: Option<&str>,
    content_type: &str,
    optional_mods: Option<&[String]>,
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
        serde_json::json!({"id":format!("{content_type}-install:{project_id}"),"kind":content_type,"operation":"install","title":title,"version":"","state":"queued","selected":true,"payload":{"projectId":project_id,"versionId":version_id,"provider":provider,"contentType":content_type,"optionalMods":optional_mods}}),
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

pub fn request_catalog_content_update(
    root: &std::path::Path,
    content_type: &str,
    project_id: &str,
    title: &str,
    provider: serde_json::Value,
    version_id: &str,
) -> Result<(), String> {
    request_catalog_content_update_if_needed(
        root,
        content_type,
        project_id,
        title,
        provider,
        version_id,
    )
    .map(|_| ())
}

pub(crate) fn request_catalog_content_update_if_needed(
    root: &std::path::Path,
    content_type: &str,
    project_id: &str,
    title: &str,
    provider: serde_json::Value,
    version_id: &str,
) -> Result<bool, String> {
    if !crate::update_check::updates_enabled(root) {
        return Err(crate::UPDATE_MODULE_DISABLED_MESSAGE.into());
    }
    if !matches!(content_type, "schematic" | "modpack") {
        return Err("unsupported ShroudEdit content update type".into());
    }
    validate_version_id(Some(project_id))?;
    validate_version_id(Some(version_id))?;
    let section = if content_type == "schematic" {
        "schematics"
    } else {
        "modpacks"
    };
    let config = shroudforge_loader_config::config::read_loader(root)?;
    let update = config["modules"]["updates"][section]["latestUpdates"]
        .as_array()
        .and_then(|updates| {
            updates.iter().find(|update| {
                update["projectId"] == project_id && update["latestVersionId"] == version_id
            })
        })
        .ok_or("The selected content update is no longer available. Check for updates again.")?;
    if content_type == "schematic" && update["modified"] == true {
        return Err("The local schematic was modified and cannot be replaced automatically".into());
    }
    if content_type == "modpack" && update["canUpdate"] != true {
        return Err("The modpack update has conflicting or changed pins".into());
    }
    enqueue_update_item(
        root,
        serde_json::json!({
            "id":format!("{content_type}-update:{project_id}"), "kind":content_type,
            "operation":"update", "title":title, "version":"", "state":"queued", "selected":true,
            "payload":{"projectId":project_id,"versionId":version_id,"provider":provider,"contentType":content_type}
        }),
    )
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
