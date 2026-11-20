use crate::*;
#[cfg(windows)]
pub fn run_scheduled_worker() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let root = args
        .windows(2)
        .find(|pair| pair[0] == "--root")
        .map(|pair| std::path::PathBuf::from(&pair[1]))
        .ok_or("missing --root")?;
    let root = root
        .canonicalize()
        .map_err(|error| format!("could not resolve update worker installation: {error}"))?;
    let Some(_worker_mutex) = acquire_queue_worker_mutex(&root)? else {
        let _ = shroudforge_logging::append(
            &root,
            'D',
            "updater",
            "A queue worker for this installation is already running",
        );
        return Ok(());
    };
    let queue_result = (|| {
        let queue = read_update_queue_value(&root);
        if queue["run"]["state"] == "running" {
            return run_queue_with_failure_handling(&root);
        }

        // Old request files are imported by versioned jobs in the one update queue.
        // They are not processed by a second worker path.
        crate::maintenance_jobs::enqueue_before_update(&root)?;
        let queue = read_update_queue_value(&root);
        let ids = queue["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["kind"] == "maintenance" && item["state"] == "queued")
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Ok(());
        }
        let updates = shroudforge_loader_config::paths::updates_dir(&root);
        let wait_for_game = updates.join("worker-queue.json").is_file();
        let wait_pid = std::fs::read(updates.join("worker-queue.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value["waitPid"].as_u64())
            .unwrap_or_default();
        crate::update_queue_storage::update_update_queue(&root, |queue| {
            if queue["run"]["state"] == "running" {
                return Err("the update queue is already running".into());
            }
            queue["run"] = serde_json::json!({
                "state": "running",
                "runId": crate::update_queue_storage::new_update_queue_run_id(),
                "waitForGame": wait_for_game,
                "waitPid": wait_pid,
                "showWindow": false,
                "itemIds": ids,
                "activeItemId": null,
                "installing": false,
                "startedAt": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });
            Ok(())
        })?;
        run_queue_with_failure_handling(&root)
    })();

    drop(_worker_mutex);
    let completed_queue = read_update_queue_value(&root);
    let uninstall_selected = completed_queue["run"]["itemIds"]
        .as_array()
        .is_some_and(|ids| {
            ids.iter().filter_map(serde_json::Value::as_str).any(|id| {
                completed_queue["items"].as_array().is_some_and(|items| {
                    items.iter().any(|item| {
                        item["id"].as_str() == Some(id)
                            && item["kind"] == "installer"
                            && item["operation"] == "uninstall"
                    })
                })
            })
        });
    if !uninstall_selected && let Err(error) = run_installed_post_install_maintenance(&root) {
        scheduled::write_status(
            &root,
            "error",
            &format!("Post-install migration failed: {error}"),
        );
        shroudforge_logging::write_activity(
            &root,
            "Updates",
            "Post-install migration",
            "Failed",
            Some(&error),
            "error",
        );
    }
    queue_result
}

#[cfg(windows)]
fn run_installed_post_install_maintenance(root: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::{io::Write, process::Stdio};

    let executable = shroudforge_loader_config::paths::updater_executable(root);
    if !executable.is_file() {
        return Err(format!(
            "installed updater executable is missing: {}",
            executable.display()
        ));
    }
    let request = serde_json::to_vec(&serde_json::json!({
        "command": "run-post-install-maintenance"
    }))
    .map_err(|error| format!("could not encode post-install maintenance request: {error}"))?;
    let mut child = std::process::Command::new(&executable)
        .args(["--request", "--root"])
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| format!("could not start the installed updater: {error}"))?;
    child
        .stdin
        .take()
        .ok_or("installed updater request stdin is unavailable")?
        .write_all(&request)
        .map_err(|error| format!("could not send post-install maintenance request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not read post-install maintenance result: {error}"))?;
    let response: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("installed updater returned an invalid response: {error}"))?;
    if !output.status.success() || response["ok"] != true {
        return Err(response["error"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                "installed updater could not finish post-install maintenance".into()
            }));
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) use shroudforge_loader_config::update_queue_lock::{acquire_queue_worker_mutex, wait_for_queue_worker_mutex, QueueWorkerMutex};

#[cfg(windows)]
fn run_queue_with_failure_handling(root: &std::path::Path) -> Result<(), String> {
    match run_update_queue(root) {
        Ok(()) => Ok(()),
        Err(error) if error == "UPDATE_CANCELLED" => {
            let ids = read_update_queue_value(root)["run"]["itemIds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            crate::finish_cancelled_queue::finish_cancelled_queue(root, &ids)
        }
        Err(error) => {
            if read_update_queue_value(root)["run"]["state"] != "running" {
                return Ok(());
            }
            record_queue_worker_failure(root, &error)
        }
    }
}

#[cfg(windows)]
pub(crate) fn record_queue_worker_failure(
    root: &std::path::Path,
    error: &str,
) -> Result<(), String> {
    let message = error.to_owned();
    let recorded = crate::update_queue_storage::update_update_queue(root, |queue| {
        if queue["run"]["state"] != "running" {
            return Ok(false);
        }
        if let Some(active_id) = queue["run"]["activeItemId"].as_str().map(str::to_owned)
            && let Some(item) = queue["items"].as_array_mut().and_then(|items| {
                items
                    .iter_mut()
                    .find(|item| item["id"].as_str() == Some(&active_id))
            })
            && !matches!(
                item["state"].as_str(),
                Some("complete" | "cancelled" | "error")
            )
        {
            item["state"] = serde_json::Value::String("error".into());
            item["message"] = serde_json::Value::String(message.clone());
        }
        queue["run"]["state"] = serde_json::Value::String("error".into());
        queue["run"]["error"] = serde_json::Value::String(message.clone());
        queue["run"]["installing"] = serde_json::Value::Bool(false);
        queue["run"]["cancelable"] = serde_json::Value::Bool(false);
        queue["run"]["finishedAt"] = serde_json::json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        Ok(true)
    })?;
    if !recorded {
        return Ok(());
    }
    scheduled::write_status(root, "error", error);
    shroudforge_logging::write_activity(
        root,
        "Updates",
        "Run update queue",
        "Failed",
        Some(error),
        "error",
    );
    Ok(())
}
#[cfg(windows)]
pub fn run_module() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let root = args
        .windows(2)
        .find(|pair| pair[0] == "--root")
        .map(|pair| std::path::PathBuf::from(&pair[1]))
        .or_else(|| {
            if args.len() != 1 {
                return None;
            }
            std::env::current_exe()
                .ok()?
                .parent()?
                .parent()
                .map(std::path::Path::to_path_buf)
        });
    if let Some(root) = root {
        match windows::hand_off_installed_process(&root) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => {
                if args.iter().any(|argument| argument == "--run-queue") {
                    return match record_queue_worker_failure(&root, &error) {
                        Ok(()) => Ok(()),
                        Err(record_error) => Err(format!(
                            "could not start the independent updater worker: {error}. Could not record this failure in the update queue: {record_error}"
                        )),
                    };
                }
                return Err(error);
            }
        }
    }
    if args
        .iter()
        .any(|argument| argument == "--purge-shroudforge-data")
    {
        let root = args
            .windows(2)
            .find(|pair| pair[0] == "--root")
            .map(|pair| std::path::PathBuf::from(&pair[1]))
            .ok_or("missing --root")?;
        let wait_pid = args
            .windows(2)
            .find(|pair| pair[0] == "--wait-pid")
            .and_then(|pair| pair[1].parse::<u32>().ok())
            .ok_or("missing or invalid --wait-pid")?;
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        return windows::run_shroudforge_data_purge_worker(&root, wait_pid, &executable);
    }
    if args.iter().any(|arg| arg == "--restore-gamefiles-worker") {
        let root = args
            .windows(2)
            .find(|pair| pair[0] == "--root")
            .map(|pair| std::path::PathBuf::from(&pair[1]))
            .ok_or("missing --root")?;
        return windows::restore_gamefiles_worker(&root);
    }
    if args.iter().any(|a| a == "--run-queue") {
        return run_scheduled_worker();
    }
    if args.iter().any(|a| a == "--open-window") {
        let root = args
            .windows(2)
            .find(|pair| pair[0] == "--root")
            .map(|pair| std::path::PathBuf::from(&pair[1]))
            .ok_or("missing --root")?;
        return run_updater_window(root);
    }
    if args.len() == 1 {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let root = executable
            .parent()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
            .filter(|root| crate::ui::host::window::is_game_root(root));
        return match root {
            Some(root) => run_updater_window(root),
            None => crate::ui::host::window::run(None),
        };
    }
    windows::run()
}

#[cfg(windows)]
fn run_updater_window(root: std::path::PathBuf) -> Result<(), String> {
    if crate::ui::host::window::is_game_root(&root) {
        let _ = crate::run_request(
            &root,
            &serde_json::json!({"command":"run-post-install-maintenance"}),
        );
    }
    crate::ui::host::window::run(Some(root))
}

#[cfg(not(windows))]
pub fn run_module() -> Result<(), String> {
    Err("ShroudForge updater is available on Windows only".into())
}
