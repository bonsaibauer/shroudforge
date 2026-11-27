use super::ModUpdateCheckLock;
use super::catalog::{
    current_game_version, fetch_cached_catalog_mod_versions, installed_catalog_packs,
    CatalogVersionStop,
};
use crate::provider_config::CatalogProviderConfig;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

fn newer_version(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    installed_version_id: &str,
    content_type: &str,
    game_version: Option<&str>,
) -> Result<Option<Value>, String> {
    let loaders = if content_type == "modpack" {
        "[\"sfpack\"]"
    } else {
        "[\"shroudtopia\",\"shroudforge\",\"eml\"]"
    };
    let mut versions = fetch_cached_catalog_mod_versions(
        root,
        client,
        provider.base_url.trim_end_matches('/'),
        project_id,
        loaders,
        CatalogVersionStop::Id(installed_version_id),
    )?;
    versions.sort_by(|left, right| {
        right["date_published"]
            .as_str()
            .unwrap_or_default()
            .cmp(left["date_published"].as_str().unwrap_or_default())
    });
    let position = versions
        .iter()
        .position(|version| version["id"].as_str() == Some(installed_version_id))
        .ok_or_else(|| format!(
            "Installed ShroudEdit {content_type} version {installed_version_id} is no longer listed"
        ))?;
    Ok(versions[..position]
        .iter()
        .find(|version| {
            version["project_id"].as_str() == Some(project_id)
                && version["project_types"].as_array().is_some_and(|types| {
                    types.iter().any(|kind| kind.as_str() == Some(content_type))
                })
                && game_version.is_none_or(|game| {
                    version["game_versions"]
                        .as_array()
                        .is_some_and(|games| games.iter().any(|value| value.as_str() == Some(game)))
                })
        })
        .cloned())
}

fn inspect_pack_changes(
    root: &Path,
    new: &shroudforge_package::modpack::ModpackManifest,
) -> Result<Vec<Value>, String> {
    let previous = shroudforge_loader_config::paths::mods_dir(root)
        .join(&new.id)
        .join("modpack.json");
    let old = shroudforge_package::modpack::parse_manifest(
        &std::fs::read(&previous).map_err(|error| format!("{}: {error}", previous.display()))?,
    )?;
    let mut changes = Vec::new();
    for item in &new.mods {
        let before = old
            .mods
            .iter()
            .find(|candidate| candidate.id() == item.id());
        let kind = match before {
            None => "added",
            Some(before)
                if serde_json::to_value(before).ok() != serde_json::to_value(item).ok() =>
            {
                "changed"
            }
            _ => "unchanged",
        };
        if kind != "unchanged" {
            changes.push(json!({"kind":kind,"modId":item.id()}));
        }
    }
    for item in &old.mods {
        if !new.mods.iter().any(|candidate| candidate.id() == item.id()) {
            changes.push(json!({"kind":"removed","modId":item.id()}));
        }
    }
    Ok(changes)
}
pub fn check_catalog_content_updates(
    root: &Path,
    content_type: &str,
    manual: bool,
) -> Result<Value, String> {
    if !matches!(content_type, "schematic" | "modpack") {
        return Err("unsupported ShroudEdit update check type".into());
    }
    let _check_lock = ModUpdateCheckLock::try_acquire(root)?
        .ok_or("A catalog update check is already running for this installation")?;
    let config = shroudforge_loader_config::config::read_loader(root)?;
    if !crate::update_check::updates_enabled(root) {
        return Err(crate::UPDATE_MODULE_DISABLED_MESSAGE.into());
    }
    if !manual
        && config
            .pointer("/modules/updates/automatic/enabled")
            .and_then(Value::as_bool)
            == Some(false)
    {
        return Err("Automatic update checks are disabled".into());
    }
    let queue_updates = config
        .pointer("/modules/updates/automatic/queueUpdates")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let provider: CatalogProviderConfig = config
        .pointer("/catalog/provider")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("invalid catalog provider: {error}"))?
        .unwrap_or_default();
    if provider.kind != "shroudedit" {
        return Err("unsupported catalog provider".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(format!(
            "ShroudForge/{} ({content_type}-update-check)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let game_version = current_game_version(root);
    let mut updates = Vec::new();
    let mut errors = Vec::new();
    let mut checked = 0usize;
    if content_type == "schematic" {
        for installed in shroudforge_schematics::schematic_library::catalog_installations(root)? {
            match newer_version(
                root,
                &client,
                &provider,
                &installed.project_id,
                &installed.version_id,
                content_type,
                game_version.as_deref(),
            ) {
                Ok(Some(candidate)) => {
                    checked += 1;
                    updates.push(json!({
                        "projectId": installed.project_id,
                        "name": installed.name,
                        "currentVersionId": installed.version_id,
                        "latestVersionId": candidate["id"],
                        "latestVersion": candidate["version_number"],
                        "modified": installed.modified,
                    }));
                }
                Ok(None) => checked += 1,
                Err(error) => errors.push(format!("{}: {error}", installed.name)),
            }
        }
    } else {
        for (project_id, installed) in installed_catalog_packs(root) {
            match newer_version(
                root,
                &client,
                &provider,
                &project_id,
                &installed.version_id,
                content_type,
                game_version.as_deref(),
            ) {
                Ok(Some(candidate)) => {
                    let Some(version_id) = candidate["id"].as_str() else {
                        errors.push(format!("{project_id}: candidate has no version ID"));
                        continue;
                    };
                    match crate::install_modpack::download_sfpack_manifest(
                        root,
                        &client,
                        &provider,
                        &project_id,
                        version_id,
                    )
                    .and_then(|pack| {
                        inspect_pack_changes(root, &pack).map(|changes| (pack, changes))
                    }) {
                        Ok((pack, changes)) => {
                            checked += 1;
                            updates.push(json!({
                                "projectId": project_id,
                                "name": pack.name,
                                "currentVersion": installed.version_number,
                                "currentVersionId": installed.version_id,
                                "latestVersion": pack.version,
                                "latestVersionId": version_id,
                                "changes": changes,
                                "conflicts": [],
                                "canUpdate": true,
                            }));
                        }
                        Err(error) => errors.push(format!("{project_id}: {error}")),
                    }
                }
                Ok(None) => checked += 1,
                Err(error) => errors.push(format!("{project_id}: {error}")),
            }
        }
    }
    let checked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let section = if content_type == "schematic" {
        "schematics"
    } else {
        "modpacks"
    };
    shroudforge_loader_config::config::update_loader(root, |config| {
        config["modules"]["updates"][section]["lastCheckAt"] = checked_at.into();
        config["modules"]["updates"][section]["latestUpdates"] = json!(updates);
        config["modules"]["updates"][section]["lastCheckResult"] = json!({
            "checked": checked,
            "available": updates.len(),
            "failed": errors.len(),
            "checkedAt": checked_at,
            "errors": errors,
        });
        Ok(())
    })?;
    let mut queued = 0usize;
    if queue_updates && content_type == "schematic" {
        let provider_value = serde_json::to_value(&provider).map_err(|error| error.to_string())?;
        for update in &updates {
            if update["modified"] == true {
                continue;
            }
            if let (Some(project_id), Some(version_id), Some(name)) = (
                update["projectId"].as_str(),
                update["latestVersionId"].as_str(),
                update["name"].as_str(),
            ) {
                match crate::request_catalog_content_update_if_needed(
                    root,
                    content_type,
                    project_id,
                    name,
                    provider_value.clone(),
                    version_id,
                ) {
                    Ok(true) => queued += 1,
                    Ok(false) => {}
                    Err(error) => errors.push(format!("{name}: {error}")),
                }
            }
        }
    }
    let result = json!({
        "checked": checked, "available": updates.len(), "queued": queued,
        "failed": errors.len(), "checkedAt": checked_at,
        "latestUpdates": updates, "errors": errors,
    });
    shroudforge_loader_config::config::update_loader(root, |config| {
        config["modules"]["updates"][section]["lastCheckAt"] = checked_at.into();
        config["modules"]["updates"][section]["latestUpdates"] = result["latestUpdates"].clone();
        config["modules"]["updates"][section]["lastCheckResult"] = json!({
            "checked": checked, "available": result["available"], "failed": errors.len(),
            "checkedAt": checked_at, "errors": errors,
        });
        Ok(())
    })?;
    shroudforge_logging::write_activity(
        root,
        "ShroudEdit",
        &format!("Check {section} updates"),
        if errors.is_empty() {
            "Succeeded"
        } else {
            "Completed with errors"
        },
        Some(&format!(
            "Checked {checked}, found {} update(s), queued {queued}, {} failure(s).",
            result["available"],
            errors.len()
        )),
        if errors.is_empty() { "info" } else { "warn" },
    );
    Ok(result)
}
