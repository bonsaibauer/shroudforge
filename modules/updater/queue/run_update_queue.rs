#[cfg(windows)]
use crate::finish_cancelled_queue::finish_cancelled_queue;
#[cfg(windows)]
use crate::scheduled;
#[cfg(windows)]
use crate::set_update_item_state::set_update_item_state;
#[cfg(windows)]
use crate::update_queue_storage::read_update_queue_value;
#[cfg(windows)]
use crate::update_queue_storage::update_update_queue;

#[cfg(windows)]
pub(crate) fn run_update_queue(root: &std::path::Path) -> Result<(), String> {
    let mut queue = read_update_queue_value(root);
    let run = queue["run"].clone();
    let ids = run["itemIds"]
        .as_array()
        .ok_or("update queue run has no item list")?
        .iter()
        .filter_map(|id| id.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let wait_for_game = run["waitForGame"].as_bool().unwrap_or(false);
    let wait_pid = run["waitPid"].as_u64().unwrap_or_default() as u32;
    let show_window = run["showWindow"].as_bool().unwrap_or(true);
    let contains_system_install = ids.iter().any(|id| {
        queue["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["id"].as_str() == Some(id.as_str())
                    && (item["kind"] == "system"
                        || (item["kind"] == "installer" && item["operation"] == "install-latest"))
            })
        })
    });
    let contains_uninstall = ids.iter().any(|id| {
        queue["items"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["id"].as_str() == Some(id.as_str())
                    && item["kind"] == "installer"
                    && item["operation"] == "uninstall"
            })
        })
    });
    shroudforge_logging::write_activity(
        root,
        "Updates",
        "Run update queue",
        "Started",
        Some(&format!("{} selected item(s).", ids.len())),
        "info",
    );
    if wait_pid != 0 {
        scheduled::wait_for_process_id(root, wait_pid)?;
    }
    if wait_for_game {
        scheduled::write_status(
            root,
            "waitingForGame",
            "Waiting for Enshrouded to close before starting selected downloads",
        );
        if let Err(error) = scheduled::wait_for_game_processes(root) {
            if error == "UPDATE_CANCELLED" {
                finish_cancelled_queue(root, &ids)?;
                return Ok(());
            }
            return Err(error);
        }
    }
    if show_window {
        scheduled::open_update_window(root)?;
    }

    let mut failures = Vec::new();
    for id in &ids {
        if let Err(error) = scheduled::check_update_cancelled(root) {
            if error == "UPDATE_CANCELLED" {
                finish_cancelled_queue(root, &ids)?;
                return Ok(());
            }
            return Err(error);
        }
        queue = read_update_queue_value(root);
        let Some(item) = queue["items"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["id"].as_str() == Some(id)))
            .cloned()
        else {
            failures.push(format!("queue item '{id}' disappeared"));
            continue;
        };
        let required_migration =
            item["kind"] == "maintenance" && crate::maintenance_jobs::is_required_migration(&item);
        if matches!(item["state"].as_str(), Some("complete" | "cancelled")) {
            shroudforge_logging::write_activity(
                root,
                "Updates",
                "Update queue item",
                "Skipped during recovery",
                Some(&format!(
                    "{} ({id}) was already {}",
                    item["title"].as_str().unwrap_or(id),
                    item["state"].as_str().unwrap_or("complete")
                )),
                "debug",
            );
            continue;
        }
        let title = item["title"].as_str().unwrap_or(id);
        shroudforge_logging::write_activity(
            root,
            "Updates",
            "Update queue item",
            "Started",
            Some(&format!("{title} ({id})")),
            "info",
        );
        update_update_queue(root, |queue| {
            queue["run"]["activeItemId"] = serde_json::Value::String(id.clone());
            queue["run"]["installing"] = serde_json::Value::Bool(false);
            queue["run"]["cancelable"] = serde_json::Value::Bool(true);
            Ok(())
        })?;
        set_update_item_state(root, id, "downloading", None)?;
        let result = match item["kind"].as_str() {
            Some("maintenance") => {
                set_update_item_state(root, id, "installing", None)?;
                crate::maintenance_jobs::execute(root, &item)
            }
            Some("system") => {
                let request = item["payload"].clone();
                let version = request["version"].as_str().unwrap_or_default();
                if item["operation"] == "install-staged" {
                    let staged =
                        shroudforge_loader_config::paths::updates_dir(root).join("pending");
                    crate::system_install::install_staged(root, &staged)
                } else {
                    scheduled::write_status_with_version(
                        root,
                        "downloading",
                        "Downloading selected ShroudForge update",
                        Some(version),
                    );
                    scheduled::stage_system_update(root, &request).and_then(|()| {
                        scheduled::check_update_cancelled(root)?;
                        set_update_item_state(
                            root,
                            id,
                            "waitingForGame",
                            Some("Download verified. Installation waits for Enshrouded to close"),
                        )?;
                        scheduled::write_status(
                            root,
                            "waitingForGame",
                            "Download verified. Waiting to install ShroudForge",
                        );
                        let staged =
                            shroudforge_loader_config::paths::updates_dir(root).join("pending");
                        crate::system_install::install_staged(root, &staged)
                    })
                }
            }
            Some("installer") if item["operation"] == "install-latest" => {
                let request = item["payload"].clone();
                let version = request["version"].as_str().unwrap_or_default();
                scheduled::write_status_with_version(
                    root,
                    "downloading",
                    "Downloading the latest ShroudForge release",
                    Some(version),
                );
                scheduled::stage_system_update(root, &request).and_then(|()| {
                    scheduled::check_update_cancelled(root)?;
                    set_update_item_state(
                        root,
                        id,
                        "waitingForGame",
                        Some("Download verified. Installation waits for Enshrouded to close"),
                    )?;
                    scheduled::write_status(
                        root,
                        "waitingForGame",
                        "Download verified. Waiting to install ShroudForge",
                    );
                    let staged =
                        shroudforge_loader_config::paths::updates_dir(root).join("pending");
                    crate::system_install::install_staged(root, &staged)
                })
            }
            Some("installer") if item["operation"] == "uninstall" => {
                set_update_item_state(
                    root,
                    id,
                    "waitingForGame",
                    Some("Waiting for Enshrouded to close before removing ShroudForge"),
                )?;
                crate::system_uninstall::uninstall_program_files(
                    root,
                    item["payload"]["restoreGamefiles"].as_bool() == Some(true),
                    item["payload"]["removeMods"].as_bool() == Some(true),
                    item["payload"]["resetUserSettings"].as_bool() == Some(true),
                    item["payload"]["purgeData"].as_bool() == Some(true),
                )
            }
            Some("mod" | "schematic" | "modpack") => {
                let payload = &item["payload"];
                let request = serde_json::json!({
                    "schemaVersion":1,
                    "queueItemId":id,
                    "operation":item["operation"],
                    "projectId":payload["projectId"],
                    "versionId":payload["versionId"],
                    "modId":payload["modId"],
                    "provider":payload["provider"]
                });
                let provider = serde_json::from_value(request["provider"].clone())
                    .map_err(|error| format!("invalid provider settings: {error}"))?;
                let project_id = request["projectId"]
                    .as_str()
                    .ok_or("mod queue item has no project ID")?;
                let update_mod_id = (item["operation"] == "update")
                    .then(|| payload["modId"].as_str())
                    .flatten();
                crate::mod_installation::install::install_catalog_content(
                    root,
                    &provider,
                    project_id,
                    payload["versionId"].as_str(),
                    update_mod_id,
                    id,
                    payload["contentType"]
                        .as_str()
                        .unwrap_or_else(|| item["kind"].as_str().unwrap_or("mod")),
                    payload["optionalMods"].as_array().map(|items| items.iter()
                        .filter_map(serde_json::Value::as_str).map(str::to_owned).collect::<Vec<_>>()).as_deref(),
                )
                .map(|_| ())
            }
            _ => Err(format!("unsupported update queue item kind for '{id}'")),
        };
        match result {
            Ok(()) => {
                let success_message =
                    if item["kind"] == "installer" && item["operation"] == "uninstall" {
                        "ShroudForge uninstallation completed"
                    } else {
                        "Update completed"
                    };
                set_update_item_state(root, id, "complete", Some(success_message))?;
                shroudforge_logging::write_activity(
                    root,
                    "Updates",
                    "Update queue item",
                    "Succeeded",
                    Some(&format!("{title} ({id})")),
                    "info",
                );
            }
            Err(error) if error == "UPDATE_CANCELLED" => {
                finish_cancelled_queue(root, &ids)?;
                return Ok(());
            }
            Err(error) => {
                set_update_item_state(root, id, "error", Some(&error))?;
                shroudforge_logging::write_activity(
                    root,
                    "Updates",
                    "Update queue item",
                    "Failed",
                    Some(&format!("{title} ({id}): {error}")),
                    "error",
                );
                failures.push(format!("{id}: {error}"));
                if required_migration {
                    break;
                }
            }
        }
    }
    update_update_queue(root, |queue| {
        queue["run"]["state"] = serde_json::Value::String("complete".into());
        queue["run"]["activeItemId"] = serde_json::Value::Null;
        queue["run"]["installing"] = serde_json::Value::Bool(false);
        queue["run"]["cancelable"] = serde_json::Value::Bool(false);
        queue["run"]["finishedAt"] = serde_json::json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        Ok(())
    })?;
    let _ = std::fs::remove_file(
        shroudforge_loader_config::paths::updates_dir(root).join("cancel-system-update"),
    );
    if failures.is_empty() {
        shroudforge_logging::write_activity(
            root,
            "Updates",
            "Run update queue",
            "Succeeded",
            Some(&format!("Completed {} selected item(s).", ids.len())),
            "info",
        );
        if contains_system_install {
            scheduled::write_status(root, "installed", "Selected update queue completed");
        } else if !contains_uninstall {
            scheduled::write_status(root, "ready", "Selected mod queue completed");
        }
        Ok(())
    } else {
        shroudforge_logging::write_activity(
            root,
            "Updates",
            "Run update queue",
            "Completed with errors",
            Some(&failures.join("; ")),
            "error",
        );
        if contains_system_install || contains_uninstall {
            scheduled::write_status(root, "error", &failures.join(", "));
        } else {
            scheduled::write_status(root, "ready", "One or more mod queue items failed");
        }
        Err(failures.join(", "))
    }
}
