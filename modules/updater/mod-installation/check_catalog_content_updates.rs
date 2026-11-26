use super::ModUpdateCheckLock;
use super::catalog::{
    InstalledCatalogPack, current_game_version, fetch_catalog_mod_versions,
    installed_catalog_packs, read_catalog_install_registry,
};
use crate::provider_config::CatalogProviderConfig;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

fn newer_version(
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
    let mut versions = fetch_catalog_mod_versions(
        client,
        provider.base_url.trim_end_matches('/'),
        project_id,
        loaders,
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
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    old: &InstalledCatalogPack,
    new: &crate::read_modpack::Modpack,
    pinned_versions: &[Value],
) -> Result<(Vec<Value>, Vec<String>), String> {
    let registry = read_catalog_install_registry(root);
    let other_packs = installed_catalog_packs(root);
    let mut changes = Vec::new();
    let mut conflicts = Vec::new();
    for member in &new.mods {
        let previous = old
            .mods
            .iter()
            .find(|previous| previous.project_id == member.project_id);
        let kind = match previous {
            None => "added",
            Some(previous) if previous.version_id != member.version_id => "changed",
            _ => "unchanged",
        };
        if kind != "unchanged" {
            changes.push(json!({
                "kind": kind,
                "projectId": member.project_id,
                "previousVersionId": previous.map(|value| value.version_id.as_str()),
                "versionId": member.version_id,
            }));
        }
        if kind == "changed" {
            conflicts.push(format!(
                "The pinned version of mod {} changed. Replace it only after resolving the installed package.",
                member.project_id
            ));
        }
        if other_packs.iter().any(|(other_id, other)| {
            other_id != project_id
                && other.mods.iter().any(|existing| {
                    existing.project_id == member.project_id
                        && existing.version_id != member.version_id
                })
        }) {
            conflicts.push(format!(
                "Mod {} is pinned differently by another installed modpack.",
                member.project_id
            ));
        }
        let slug =
            crate::install_catalog_mod::catalog_project_slug(client, provider, &member.project_id)?;
        if let Some(local) = shroudforge_package::package_lookup::find_mod_package(root, &slug) {
            let recorded = &registry["projects"][member.project_id.as_str()];
            if recorded["versionId"].as_str() != Some(member.version_id.as_str())
                || recorded["modId"].as_str() != Some(slug.as_str())
            {
                conflicts.push(format!(
                    "Installed mod {slug} does not match the version required by this modpack."
                ));
            } else {
                let manifest = shroudforge_package::config::read_manifest_path(root, &local)
                    .map_err(|error| format!("Could not read installed mod {slug}: {error}"))?;
                let published_number = pinned_versions
                    .iter()
                    .find(|version| version["id"].as_str() == Some(member.version_id.as_str()))
                    .and_then(|version| version["version_number"].as_str());
                let local_number = manifest.version.to_string();
                if manifest.id != slug || published_number != Some(local_number.as_str()) {
                    conflicts.push(format!(
                        "Installed mod {slug} differs from its pinned ShroudEdit package version."
                    ));
                }
            }
        }
    }
    for member in &old.mods {
        if !new
            .mods
            .iter()
            .any(|candidate| candidate.project_id == member.project_id)
        {
            changes.push(json!({
                "kind": "removed",
                "projectId": member.project_id,
                "previousVersionId": member.version_id,
                "versionId": null,
            }));
        }
    }
    Ok((changes, conflicts))
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
                    match crate::install_catalog_mod::download_sfpack_manifest(
                        root,
                        &client,
                        &provider,
                        &project_id,
                        version_id,
                    )
                    .and_then(|(pack, pinned_versions)| {
                        inspect_pack_changes(
                            root,
                            &client,
                            &provider,
                            &project_id,
                            &installed,
                            &pack,
                            &pinned_versions,
                        )
                        .map(|(changes, conflicts)| (pack, changes, conflicts))
                    }) {
                        Ok((pack, changes, conflicts)) => {
                            checked += 1;
                            let can_update = conflicts.is_empty();
                            updates.push(json!({
                                "projectId": project_id,
                                "name": pack.name,
                                "currentVersion": installed.version_number,
                                "currentVersionId": installed.version_id,
                                "latestVersion": pack.version_id,
                                "latestVersionId": version_id,
                                "changes": changes,
                                "conflicts": conflicts,
                                "canUpdate": can_update,
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
