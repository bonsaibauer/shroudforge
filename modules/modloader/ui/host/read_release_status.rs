use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

pub(crate) fn read_release_status(
    root: &Path,
    central: &serde_json::Value,
    release: &Arc<Mutex<serde_json::Value>>,
) -> serde_json::Value {
    let mut snapshot = release
        .lock()
        .map(|value| value.clone())
        .unwrap_or_else(|_| idle_release_status(root));
    snapshot["updaterAvailable"] = shroudforge_loader_config::paths::updater_available(root).into();
    snapshot["installationDetected"] =
        shroudforge_loader_config::paths::shroudforge_program_files_present(root).into();
    if let Some(latest) = central.pointer("/modules/updates/system/latestRelease") {
        if latest.is_object()
            && latest["version"].is_string()
            && latest["baseVersion"].is_string()
            && latest["build"].as_u64().is_some()
            && latest["downloadUrl"].is_string()
            && latest["checksum"].is_string()
        {
            let update_available = latest["updateAvailable"].as_bool().unwrap_or(false);
            snapshot["latestVersion"] = latest["version"].clone();
            snapshot["updateAvailable"] = update_available.into();
            snapshot["mandatory"] = latest["mandatory"].as_bool().unwrap_or(false).into();
            snapshot["prerelease"] = latest["prerelease"].as_bool().unwrap_or(false).into();
            snapshot["releaseUrl"] = latest["releaseUrl"].clone();
            if update_available {
                snapshot["message"] = latest["message"].clone();
            }
            if matches!(snapshot["state"].as_str(), Some("idle" | "ready")) {
                snapshot["state"] = "ready".into();
                snapshot["message"] = if update_available {
                    latest["message"].clone()
                } else {
                    "ShroudForge is up to date.".into()
                };
            }
        }
    }

    let status_path =
        shroudforge_loader_config::paths::updates_dir(root).join("updater-status.json");
    if let Ok(bytes) = fs::read(status_path) {
        if let Ok(status) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            let terminal = matches!(
                status["status"].as_str(),
                Some("installed" | "uninstalled" | "error" | "cancelled")
            );
            let last_check = central["modules"]["updates"]["system"]["lastCheckAt"].as_u64();
            let superseded = terminal
                && (snapshot["updateAvailable"] == true
                    || matches!(snapshot["state"].as_str(), Some("checking" | "error"))
                    || status["updatedAt"]
                        .as_u64()
                        .zip(last_check)
                        .is_some_and(|(updated, checked)| updated < checked));
            if status["operation"] == "systemStage" && !superseded {
                if let Some(state) = status["status"].as_str() {
                    let staged = shroudforge_loader_config::paths::updates_dir(root)
                        .join("pending.ready")
                        .is_file();
                    snapshot["state"] = if state == "installed" || (state == "staged" && !staged) {
                        "idle"
                    } else {
                        state
                    }
                    .into();
                    snapshot["message"] = status["message"].clone();
                    snapshot["staged"] = (state == "staged" && staged).into();
                    if status["version"].is_string() {
                        snapshot["latestVersion"] = status["version"].clone();
                    }
                    snapshot["downloadedBytes"] =
                        status["downloadedBytes"].as_u64().unwrap_or(0).into();
                    snapshot["totalBytes"] = status["totalBytes"].clone();
                    snapshot["bytesPerSecond"] = status["bytesPerSecond"].clone();
                    if state == "installed" {
                        snapshot["currentVersion"] = shroudforge_package::installed_version(root).into();
                    }
                }
            }
        }
    }
    snapshot
}

pub(crate) fn idle_release_status(root: &Path) -> serde_json::Value {
    serde_json::json!({
        "currentVersion": shroudforge_package::installed_version(root),
        "updaterAvailable": shroudforge_loader_config::paths::updater_available(root),
        "latestVersion": null,
        "updateAvailable": false,
        "mandatory": false,
        "prerelease": false,
        "state": "idle",
        "message": null,
        "releaseUrl": null,
        "staged": shroudforge_loader_config::paths::updates_dir(root)
            .join("pending.ready")
            .is_file(),
        "downloadedBytes": 0,
        "totalBytes": null,
        "bytesPerSecond": null
    })
}
