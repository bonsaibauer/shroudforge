#[path = "system-updates/notes.rs"]
mod system_updates_notes;
pub use system_updates_notes::*;

#[path = "system-updates/repair.rs"]
mod system_updates_repair;
pub use system_updates_repair::*;

#[path = "system-updates/headless_check.rs"]
mod system_updates_headless_check;
pub use system_updates_headless_check::*;

#[path = "system-updates/requests.rs"]
mod system_updates_requests;
pub use system_updates_requests::*;

#[path = "system-updates/queue_installation.rs"]
mod queue_installation;
pub use queue_installation::{queue_latest_installation, queue_uninstall};

#[path = "mod-installation/requests.rs"]
mod mod_installation_requests;
pub use mod_installation_requests::*;

mod download_response_body;

#[path = "mod-installation/install_catalog_schematic.rs"]
mod install_catalog_schematic;
#[path = "mod-installation/read_modpack.rs"]
mod read_modpack;
#[path = "mod-installation/install_modpack.rs"]
mod install_modpack;

#[path = "windows/entry.rs"]
mod windows_entry;
pub use windows_entry::*;

#[path = "restore_gamefiles.rs"]
mod restore_gamefiles;
pub use restore_gamefiles::request_gamefiles_restore;

#[cfg(windows)]
#[path = "windows/worker.rs"]
mod windows;

#[cfg(windows)]
#[path = "windows/restore_gamefiles.rs"]
mod restore_worker;

#[cfg(windows)]
#[path = "system-updates/install_system_update.rs"]
mod system_install;

#[cfg(windows)]
#[path = "system-updates/uninstall_system.rs"]
mod system_uninstall;

#[cfg(windows)]
#[path = "system-updates/stage_system_update.rs"]
mod system_stage;

#[cfg(windows)]
#[path = "windows/scheduler.rs"]
mod scheduled;

#[path = "system-updates/check.rs"]
pub mod configured_check;
#[path = "mod-installation/mod.rs"]
pub mod mod_installation;
pub mod ui;

pub mod documents;
#[path = "system-updates/check_lock.rs"]
pub mod update_check;

pub mod catalog_item;
pub mod provider_config;

mod recycle_bin;

pub(crate) const UPDATE_MODULE_DISABLED_MESSAGE: &str = "Updates module is disabled in Settings. The update service is unavailable until it is enabled.";

/// Run recycle-bin restores from an updater copy outside the managed install
/// files so Windows can restore a saved updater executable.
pub fn hand_off_request_to_worker(
    root: &std::path::Path,
    request: &serde_json::Value,
    input: &[u8],
) -> Result<bool, String> {
    #[cfg(windows)]
    let result = windows::hand_off_request_worker(root, request, input);
    #[cfg(not(windows))]
    let result = {
        let _ = (root, request, input);
        Ok(false)
    };
    if let Err(error) = &result {
        let _ = shroudforge_logging::append(
            root,
            'E',
            "updater",
            &format!("Could not prepare updater request worker: {error}"),
        );
    }
    result
}

pub fn wait_for_request_parent(root: &std::path::Path, pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    return scheduled::wait_for_process_id(root, pid);
    #[cfg(not(windows))]
    {
        let _ = (root, pid);
        Ok(())
    }
}

/// Execute one update request in the updater process.
pub fn run_request(
    root: &std::path::Path,
    request: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    if !root.is_dir() {
        return Err("updater request root is not a directory".into());
    }
    #[cfg(windows)]
    if let Err(error) = maintenance_jobs::run_after_install(root) {
        let _ = shroudforge_logging::append(
            root,
            'E',
            "updater",
            &format!("Updater request stopped because post-install maintenance failed: {error}"),
        );
        return Err(error);
    }
    let command = request
        .get("command")
        .and_then(serde_json::Value::as_str)
        .ok_or("updater request is missing its command")?;
    if command == "run-post-install-maintenance" {
        return Ok(serde_json::json!({"completed":true}));
    }
    let details = request
        .get("id")
        .or_else(|| request.get("modId"))
        .and_then(serde_json::Value::as_str)
        .map(|id| format!(" {id}"))
        .unwrap_or_default();
    if !matches!(
        command,
        "queue-latest-installation"
            | "queue-uninstall"
            | "start-update-queue"
            | "recycle-game-asset-backups"
            | "convert-schematic"
            | "convert-automatic-schematics"
    ) && let Err(error) = updates_module_status(root)
    {
        let disabled = error == UPDATE_MODULE_DISABLED_MESSAGE;
        let _ = shroudforge_logging::append(
            root,
            if disabled { 'W' } else { 'E' },
            "updater",
            &if disabled {
                format!("Updater request {command}{details} unavailable: {error}")
            } else {
                format!("Updater request {command}{details} failed: {error}")
            },
        );
        return Err(error);
    }
    let result: Result<serde_json::Value, String> = (|| match command {
        "check-system-update" => {
            #[cfg(windows)]
            {
                let mode = if request["manual"].as_bool().unwrap_or(true) {
                    update_check::SystemUpdateCheckMode::Manual
                } else {
                    update_check::SystemUpdateCheckMode::Automatic
                };
                headless_check_system_updates(root, mode).map(|message| {
                    let release = shroudforge_loader_config::config::read_loader(root)
                        .ok()
                        .and_then(|config| {
                            config
                                .pointer("/modules/updates/system/latestRelease")
                                .cloned()
                        });
                    let success_message = match release {
                        Some(value) if value["updateAvailable"].as_bool() == Some(true) => {
                            "updates.check.available"
                        }
                        Some(value) if value.is_null() => "updates.check.none",
                        _ if message.contains("No compatible") => "updates.check.none",
                        _ => "updates.check.upToDate",
                    };
                    serde_json::json!({
                        "message": message,
                        "successMessage": success_message
                    })
                })
            }
            #[cfg(not(windows))]
            {
                Err("system update checks are unavailable on this platform".into())
            }
        }
        "check-mod-updates" => mod_installation::catalog::check_catalog_mod_updates(
            root,
            request["manual"].as_bool().unwrap_or(true),
        )
        .map(|mut result| {
            result["successMessage"] = if result["available"].as_u64().unwrap_or_default() > 0 {
                "updates.mods.found".into()
            } else {
                "updates.mods.checked".into()
            };
            result
        }),
        "check-all-updates" => {
            let mut errors = Vec::new();
            for check in [
                "check-system-update",
                "check-mod-updates",
                "check-schematic-updates",
                "check-modpack-updates",
            ] {
                if let Err(error) =
                    run_request(root, &serde_json::json!({"command":check,"manual":true}))
                {
                    errors.push(format!("{check}: {error}"));
                }
            }
            if errors.is_empty() {
                Ok(serde_json::json!({"successMessage":"updates.mods.checked"}))
            } else {
                Err(errors.join("; "))
            }
        }
        "check-schematic-updates" | "check-modpack-updates" => {
            let content_type = if command == "check-schematic-updates" {
                "schematic"
            } else {
                "modpack"
            };
            mod_installation::check_catalog_content_updates::check_catalog_content_updates(
                root,
                content_type,
                request["manual"].as_bool().unwrap_or(true),
            )
            .map(|mut result| {
                result["successMessage"] = "updates.mods.checked".into();
                result
            })
        }
        "install-system-update" => (|| {
            let release = checked_system_release(root)?;
            let version = request_string(&release, "version")?.to_owned();
            request_system_update(root, release, true, false)?;
            Ok(serde_json::json!({"queued":true,"version":version,"close":true}))
        })(),
        "install-staged-update" => {
            let wait_pid = request["waitPid"].as_u64().unwrap_or_default() as u32;
            request_staged_install(root, wait_pid).map(|()| serde_json::json!({"queued":true}))
        }
        "search-catalog" => (|| {
            if !update_check::updates_enabled(root) {
                return Err(UPDATE_MODULE_DISABLED_MESSAGE.into());
            }
            let provider = request
                .get("provider")
                .cloned()
                .ok_or_else(|| "catalog search request is missing its provider".to_owned())
                .and_then(|value| {
                    serde_json::from_value::<provider_config::CatalogProviderConfig>(value)
                        .map_err(|error| format!("invalid catalog provider: {error}"))
                })?;
            let search =
                serde_json::from_value::<mod_installation::catalog::CatalogSearchParameters>(
                    request.clone(),
                )
                .map_err(|error| format!("invalid catalog search filters: {error}"))?;
            let result = mod_installation::catalog::fetch_catalog(root, &provider, search)?;
            serde_json::to_value(result).map_err(|error| error.to_string())
        })(),
        "load-catalog-project" => (|| {
            if !update_check::updates_enabled(root) {
                return Err(UPDATE_MODULE_DISABLED_MESSAGE.into());
            }
            let provider = read_request_provider(request)?;
            let project_id = request_string(request, "projectId")?;
            mod_installation::catalog::fetch_catalog_project(root, &provider, project_id)
        })(),
        "preview-modpack" => (|| {
            let provider = read_request_provider(request)?;
            let project_id = request_string(request, "projectId")?;
            let version_id = request_string(request, "versionId")?;
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build().map_err(|error| error.to_string())?;
            let pack = install_modpack::download_sfpack_manifest(root, &client, &provider, project_id, version_id)?;
            let mut preview = serde_json::to_value(pack).map_err(|error| error.to_string())?;
            preview["catalogVersionId"] = version_id.into();
            Ok(preview)
        })(),
        "install-mod" => (|| {
            let provider = read_request_provider(request)?;
            let provider = serde_json::to_value(provider)
                .map_err(|error| format!("could not serialize catalog provider: {error}"))?;
            let project_id = request_string(request, "projectId")?;
            let title = request
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(project_id);
            let version_id = request.get("versionId").and_then(serde_json::Value::as_str);
            let content_type = request
                .get("contentType")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("mod");
            let optional_mods = request.get("optionalMods").and_then(serde_json::Value::as_array)
                .map(|items| items.iter().map(|item| item.as_str().map(str::to_owned).ok_or("optional mod ID must be text")).collect::<Result<Vec<_>, _>>())
                .transpose()?;
            request_mod_install(root, project_id, title, provider, version_id, content_type, optional_mods.as_deref())?;
            Ok(
                serde_json::json!({"message":format!("Mod installation for {project_id} added to the update queue.")}),
            )
        })(),
        "update-mod" => (|| {
            let provider = read_request_provider(request)?;
            let provider_value = serde_json::to_value(&provider)
                .map_err(|error| format!("could not serialize catalog provider: {error}"))?;
            let mod_id = request_string(request, "modId")?;
            let title = request
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(mod_id);
            let project_id = mod_installation::catalog::ensure_catalog_project_for_installed_mod(
                root, &provider, mod_id,
            )?
            .ok_or("No ShroudEdit project was found for this mod ID.")?;
            let version_id = request.get("versionId").and_then(serde_json::Value::as_str);
            request_mod_update(root, &project_id, mod_id, title, provider_value, version_id)?;
            Ok(serde_json::json!({"message":format!("Update queued for {mod_id}.")}))
        })(),
        "update-schematic" | "update-modpack" => (|| {
            let content_type = if command == "update-schematic" {
                "schematic"
            } else {
                "modpack"
            };
            let provider = read_request_provider(request)?;
            let provider_value = serde_json::to_value(provider)
                .map_err(|error| format!("could not serialize catalog provider: {error}"))?;
            let project_id = request_string(request, "projectId")?;
            let version_id = request_string(request, "versionId")?;
            let title = request
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(project_id);
            request_catalog_content_update(
                root,
                content_type,
                project_id,
                title,
                provider_value,
                version_id,
            )?;
            Ok(serde_json::json!({"queued":true,"message":"updates.queue.added"}))
        })(),
        "stage-system-update" => (|| {
            if shroudforge_loader_config::config::read_loader(root)?
                .pointer("/modules/updates/system/latestRelease")
                .is_none_or(|release| !release.is_object())
            {
                headless_check_system_updates(
                    root,
                    update_check::SystemUpdateCheckMode::Automatic,
                )?;
            }
            let release = checked_system_release(root)?;
            let wait_for_game = request["waitForGame"].as_bool().unwrap_or(true);
            request_system_update(root, release.clone(), wait_for_game, true)?;
            Ok(serde_json::json!({
                "message":format!("Download of version {} queued in the update queue.", release["version"].as_str().unwrap_or_default())
            }))
        })(),
        "enqueue-system-update" => checked_system_release(root)
            .and_then(|release| enqueue_system_update(root, release))
            .map(|()| serde_json::json!({"queued":true})),
        "queue-latest-installation" => queue_latest_installation(root),
        "queue-uninstall" => queue_uninstall(
            root,
            request["restoreGamefiles"].as_bool() == Some(true),
            request["removeMods"].as_bool() == Some(true),
            request["resetUserSettings"].as_bool() == Some(true),
            request["purgeData"].as_bool() == Some(true),
        ),
        "select-update-queue-items" => (|| {
            let ids = request
                .get("ids")
                .and_then(serde_json::Value::as_array)
                .ok_or("update selection request is missing its item IDs")?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or("update item IDs must be strings")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let selected = request["selected"]
                .as_bool()
                .ok_or("update selection request is missing its selected value")?;
            select_update_queue_items(root, &ids, selected)?;
            Ok(serde_json::json!({"selected":selected}))
        })(),
        "remove-update-queue-item" => {
            let id = request_string(request, "id")?;
            remove_update_queue_item(root, id).map(|()| serde_json::json!({"removed":true}))
        }
        "start-update-queue" => (|| {
            let ids = match request.get("ids") {
                Some(value) => value
                    .as_array()
                    .ok_or("update queue IDs must be an array")?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(str::to_owned)
                            .ok_or("update queue IDs must be strings")
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            };
            start_update_queue(
                root,
                &ids,
                request["waitForGame"].as_bool().unwrap_or(true),
                request["showWindow"].as_bool().unwrap_or(true),
            )?;
            Ok(serde_json::json!({"started":true}))
        })(),
        "clear-update-queue" => {
            clear_update_queue(root).map(|()| serde_json::json!({"cleared":true}))
        }
        "cancel-update" => {
            cancel_update_queue(root).map(|()| serde_json::json!({"cancelled":true}))
        }
        "restore-gamefiles" => request_gamefiles_restore(root).map(|()| {
            serde_json::json!({
                "message":"Gamefile restoration was handed to the updater.",
                "successMessage":"backups.restore.queued"
            })
        }),
        "capture-gamefile-originals" => {
            #[cfg(windows)]
            {
                capture_original_game_files(root, request)
                    .map(|message| serde_json::json!({"message":message}))
            }
            #[cfg(not(windows))]
            {
                Err("gamefile capture is available on Windows only".into())
            }
        }
        "capture-verified-gamefile-originals" => {
            #[cfg(windows)]
            {
                capture_verified_original_game_files(root, request).map(|message| {
                    serde_json::json!({
                        "message":message,
                        "successMessage":"backups.originals.captured"
                    })
                })
            }
            #[cfg(not(windows))]
            {
                Err("gamefile capture is available on Windows only".into())
            }
        }
        "recycle-game-asset-backups" => {
            let target = request_string(request, "target")?;
            recycle_bin::recycle_game_asset_backups(root, target)
                .map(|entry| serde_json::json!({"entry":entry}))
        }
        "convert-schematic" => {
            let name = request_string(request, "id")?;
            recycle_bin::convert_schematic_to_latest(root, name).map(
                |()| serde_json::json!({"converted":true,"successMessage":"schematics.updated"}),
            )
        }
        "convert-automatic-schematics" => recycle_bin::convert_automatic_schematics(root)
            .map(|()| serde_json::json!({"converted":true})),
        "recycle-bin-list" => (|| {
            let entries = recycle_bin::list_entries(root)?;
            Ok(serde_json::json!({"entries": entries}))
        })(),
        "recycle-bin-restore" => {
            let id = request
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or("recycle-bin restore request is missing its entry ID")?;
            recycle_bin::restore_entry(root, id).map(|()| serde_json::json!({"restored": true}))
        }
        "recycle-bin-delete" => {
            let id = request
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or("recycle-bin delete request is missing its entry ID")?;
            recycle_bin::delete_entry(root, id).map(|()| serde_json::json!({"deleted": true}))
        }
        "recycle-bin-empty" => {
            recycle_bin::empty(root).map(|removed| serde_json::json!({"removed": removed}))
        }
        "recycle-mod" => {
            let id = request
                .get("modId")
                .and_then(serde_json::Value::as_str)
                .ok_or("recycle mod request is missing its mod ID")?;
            recycle_bin::recycle_mod(root, id).map(|()| serde_json::json!({"removed": true}))
        }
        "recycle-modpack" => {
            let id = request.get("modId").and_then(serde_json::Value::as_str)
                .ok_or("recycle modpack request is missing its pack ID")?;
            recycle_bin::recycle_modpack(root, id).map(|()| serde_json::json!({"removed": true}))
        }
        _ => Err(format!("unsupported updater request: {command}")),
    })();
    if command != "check-mod-updates" || result.is_err() {
        match &result {
            Ok(_) => {
                let _ = shroudforge_logging::append(
                    root,
                    'I',
                    "updater",
                    &format!("Completed updater request {command}{details}"),
                );
            }
            Err(error) => {
                let disabled = error == UPDATE_MODULE_DISABLED_MESSAGE;
                let _ = shroudforge_logging::append(
                    root,
                    if disabled { 'W' } else { 'E' },
                    "updater",
                    &if disabled {
                        format!("Updater request {command}{details} unavailable: {error}")
                    } else {
                        format!("Updater request {command}{details} failed: {error}")
                    },
                );
            }
        }
    }
    result
}

fn updates_module_status(root: &std::path::Path) -> Result<(), String> {
    let config = shroudforge_loader_config::config::read_loader(root)
        .map_err(|error| format!("Could not read ShroudForge module settings: {error}"))?;
    if config
        .pointer("/modules/updates/enabled")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        Ok(())
    } else {
        Err(UPDATE_MODULE_DISABLED_MESSAGE.into())
    }
}

fn request_string<'a>(request: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    request
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("updater request is missing '{key}'"))
}

fn read_request_provider(
    request: &serde_json::Value,
) -> Result<provider_config::CatalogProviderConfig, String> {
    request
        .get("provider")
        .cloned()
        .ok_or_else(|| "mod update request is missing its provider".to_owned())
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|error| format!("invalid catalog provider: {error}"))
        })
}

fn checked_system_release(root: &std::path::Path) -> Result<serde_json::Value, String> {
    if !update_check::updates_enabled(root) {
        return Err(UPDATE_MODULE_DISABLED_MESSAGE.into());
    }
    shroudforge_loader_config::config::read_loader(root)?
        .pointer("/modules/updates/system/latestRelease")
        .cloned()
        .filter(|release| release.is_object() && release["updateAvailable"].as_bool() == Some(true))
        .ok_or_else(|| "Check for system updates before adding one to the queue.".into())
}

mod maintenance_jobs;

pub mod release_state;

pub mod release;

pub mod restore_checked_release;
pub use restore_checked_release::restore_persisted_update_result;

#[path = "system-updates/read_installed_release.rs"]
mod read_installed_release;
pub use read_installed_release::installed_release;
pub use read_installed_release::installed_version;

#[path = "system-updates/fetch_system_release.rs"]
mod fetch_system_release;
pub use fetch_system_release::fetch_latest;
pub use fetch_system_release::fetch_latest_github;

#[path = "system-updates/compare_release_versions.rs"]
mod compare_release_versions;
pub use compare_release_versions::is_update_available;
pub use compare_release_versions::parse_release_tag;

#[path = "system-updates/github_release.rs"]
mod github_release;

#[path = "queue/update_queue_storage.rs"]
mod update_queue_storage;
pub(crate) use update_queue_storage::read_update_queue_value;
pub(crate) use update_queue_storage::try_read_update_queue_value;

#[path = "queue/read_update_queue.rs"]
mod read_update_queue;
pub use read_update_queue::read_update_queue;

#[path = "queue/set_update_item_state.rs"]
mod set_update_item_state;
pub use set_update_item_state::set_active_queue_item_state;

#[path = "queue/add_update_item.rs"]
mod add_update_item;
pub(crate) use add_update_item::enqueue_update_item;

#[path = "queue/select_update_items.rs"]
mod select_update_items;
pub use select_update_items::select_update_queue_item;
pub use select_update_items::select_update_queue_items;

#[path = "queue/remove_update_item.rs"]
mod remove_update_item;
pub use remove_update_item::remove_update_queue_item;

#[path = "queue/clear_update_queue.rs"]
mod clear_update_queue;
pub use clear_update_queue::clear_update_queue;

#[path = "queue/cancel_update_queue.rs"]
mod cancel_update_queue;
pub use cancel_update_queue::cancel_update_queue;

#[path = "queue/start_update_queue.rs"]
mod start_update_queue;
pub use start_update_queue::start_update_queue;

#[path = "queue/run_update_queue.rs"]
mod run_update_queue;
#[cfg(windows)]
pub(crate) use run_update_queue::run_update_queue;

#[path = "queue/finish_cancelled_queue.rs"]
mod finish_cancelled_queue;

#[path = "control/request_runtime_mod_action.rs"]
mod request_runtime_mod_action;
pub use request_runtime_mod_action::request_runtime_mod_reload;
pub use request_runtime_mod_action::request_runtime_mod_unload;

#[path = "control/run_headless_update_controller.rs"]
mod run_headless_update_controller;
pub use run_headless_update_controller::run_headless_control_controller;

#[path = "control/claim_update_actions.rs"]
mod claim_update_actions;

#[path = "control/execute_update_action.rs"]
mod execute_update_action;

#[path = "control/complete_update_action.rs"]
mod complete_update_action;

#[path = "control/update_control_action.rs"]
mod update_control_action;

#[cfg(windows)]
#[path = "system-updates/write_update_status.rs"]
mod write_update_status;

#[cfg(windows)]
#[path = "system-updates/check_update_cancelled.rs"]
mod check_update_cancelled;

#[cfg(windows)]
#[path = "system-updates/clear_staged_update.rs"]
mod clear_staged_update;

#[cfg(windows)]
#[path = "system-updates/read_install_arguments.rs"]
mod read_install_arguments;

#[path = "system-updates/validate_update_paths.rs"]
mod validate_update_paths;

#[path = "system-updates/read_managed_release_files.rs"]
mod read_managed_release_files;

#[path = "system-updates/apply_release_files.rs"]
mod apply_release_files;

#[path = "system-updates/restore_release_files.rs"]
mod restore_release_files;

#[path = "system-updates/remove_release_files.rs"]
mod remove_release_files;

#[path = "control/capture_original_game_files.rs"]
mod capture_original_game_files;
#[cfg(windows)]
pub(crate) use capture_original_game_files::capture_original_game_files;
#[cfg(windows)]
pub(crate) use capture_original_game_files::capture_verified_original_game_files;

#[path = "control/read_action_parameters.rs"]
mod read_action_parameters;

#[path = "control/check_due_system_update.rs"]
mod check_due_system_update;
pub(crate) use check_due_system_update::system_update_check_delay;
pub(crate) use check_due_system_update::{check_due_mod_updates, mod_update_check_delay};

#[path = "mod-installation/install_catalog_mod.rs"]
mod install_catalog_mod;

#[path = "mod-installation/download_mod_package.rs"]
mod download_mod_package;

#[path = "mod-installation/stage_mod_in_storage.rs"]
mod stage_mod_in_storage;

#[path = "mod-installation/replace_installed_mod.rs"]
mod replace_installed_mod;

#[path = "mod-installation/verify_mod_checksum.rs"]
mod verify_mod_checksum;

#[path = "mod-installation/extract_mod_archive.rs"]
mod extract_mod_archive;

#[path = "mod-installation/mod_package.rs"]
mod mod_package;
