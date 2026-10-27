use crate::validate_update_paths::{
    installed_path, mod_relative_path, validate_file_path, validate_release_path,
};
use std::os::windows::fs::MetadataExt;
use std::{fs, path::Path};

const VERSION_MANIFEST: &str = "shroudforge/version.json";
const INJECTION_FILES: &[&str] = &["dbghelp.dll", "dinput8.dll", "winmm.dll"];
const FALLBACK_PROGRAM_FILES: &[&str] = &[
    "shroudforge/shroudforge.exe",
    "shroudforge/shroudforge-updater.exe",
    "shroudforge/shroudforge-runtime.dll",
    "shroudforge/kfc-runtime.dll",
    VERSION_MANIFEST,
];

pub(crate) fn uninstall_program_files(
    root: &Path,
    restore_gamefiles: bool,
    remove_mods: bool,
    reset_user_settings: bool,
    purge_data: bool,
) -> Result<(), String> {
    let reset_user_settings = reset_user_settings && !purge_data;
    log_uninstall(
        root,
        'I',
        &format!(
            "Starting ShroudForge uninstall. restore_gamefiles={restore_gamefiles}, remove_mods={remove_mods}, reset_user_settings={reset_user_settings}, purge_data={purge_data}"
        ),
    );
    crate::scheduled::write_status(
        root,
        "waitingForGame",
        "Waiting for Enshrouded to close before removing ShroudForge",
    );
    crate::scheduled::wait_for_game_processes(root)?;
    crate::scheduled::check_update_cancelled(root)?;

    let (paths, used_fallback) = program_paths(root, remove_mods)?;
    log_uninstall(
        root,
        'D',
        &format!("Uninstall plan contains {} file(s)", paths.len()),
    );
    for relative in &paths {
        validate_file_path(root, relative)?;
    }
    crate::scheduled::prepare_release_file_locks(root, &paths)?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_id = format!("system-uninstall-{stamp}-{}", std::process::id());
    let backup_entry = shroudforge_loader_config::paths::recycle_bin_dir(root).join(&backup_id);
    let backup = crate::recycle_bin::preserve_update_files_with_progress_and_cancel(
        root,
        &backup_id,
        &format!(
            "ShroudForge uninstall. Mods selected={remove_mods}, settings reset selected={reset_user_settings}"
        ),
        &paths,
        Some(root),
        |completed, total, relative, copied, file_total| {
            crate::scheduled::check_update_cancelled(root)?;
            crate::write_update_status::write_file_progress(
                root,
                "installing",
                "Preparing a recoverable copy of ShroudForge files",
                completed,
                total,
                relative,
                copied,
                file_total,
            );
            Ok(())
        },
    )?;
    log_uninstall(
        root,
        'I',
        &format!(
            "Created uninstall recovery copy at {}",
            backup_entry.display()
        ),
    );
    if let Err(error) = ensure_uninstall_can_continue(
        root,
        restore_gamefiles,
        remove_mods,
        reset_user_settings,
        purge_data,
    ) {
        return Err(discard_unused_recovery_copy(root, &backup_entry, error));
    }

    if restore_gamefiles {
        crate::set_active_queue_item_state(
            root,
            "installer:uninstall",
            "restoring",
            Some("Restoring original Enshrouded game files"),
        )?;
        let target = if root.join("enshrouded_server.exe").is_file() {
            "server"
        } else {
            "client"
        };
        let restore_result = crate::restore_gamefiles::disable_all_mods_for_gamefile_restore(root)
            .and_then(|()| {
                crate::restore_gamefiles::restore_original_gamefiles(root, target).map(|_| ())
            });
        if let Err(error) = restore_result {
            return Err(discard_unused_recovery_copy(root, &backup_entry, error));
        }
        set_uninstall_cancelable(root, true)?;
        if let Err(error) = crate::scheduled::check_update_cancelled(root) {
            return Err(discard_unused_recovery_copy(root, &backup_entry, error));
        }
    }

    let action_message = uninstall_action_message(
        restore_gamefiles,
        remove_mods,
        reset_user_settings,
        purge_data,
    );
    crate::set_active_queue_item_state(
        root,
        "installer:uninstall",
        "uninstalling",
        Some(&action_message),
    )?;
    let result = (|| {
        for relative in &paths {
            crate::scheduled::check_update_cancelled(root)?;
            let path = installed_path(root, relative);
            crate::remove_release_files::remove_entry(&path)?;
            remove_empty_program_directories(root, &path);
            if remove_mods && mod_relative_path(relative).is_some() {
                crate::remove_release_files::remove_empty_mod_directories(root, &path)?;
            }
        }
        crate::scheduled::check_update_cancelled(root)?;
        if reset_user_settings {
            reset_loader_settings(root)?;
        }
        if purge_data {
            crate::windows::schedule_shroudforge_data_purge(root)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let error = if restore_gamefiles {
            format!("{error}. Original game files were restored before removal failed")
        } else {
            error
        };
        log_uninstall(
            root,
            'E',
            &format!("File removal failed: {error}. Starting rollback"),
        );
        crate::scheduled::write_status(
            root,
            "restoring",
            &format!("Removal failed. Restoring ShroudForge files: {error}"),
        );
        if let Err(restore_error) = crate::restore_release_files::restore_with_progress(
            &backup,
            root,
            &paths,
            |completed, total, relative, copied, file_total| {
                crate::write_update_status::write_file_progress(
                    root,
                    "restoring",
                    "Restoring ShroudForge files",
                    completed,
                    total,
                    relative,
                    copied,
                    file_total,
                );
                Ok(())
            },
        ) {
            let detail = format!(
                "{error}. Restoring removed ShroudForge files also failed: {restore_error}. The recovery copy remains at {}",
                backup_entry.display()
            );
            log_uninstall(root, 'E', &detail);
            return Err(detail);
        }
        log_uninstall(
            root,
            'W',
            &format!("Removal failed, but rollback restored the files: {error}"),
        );
        if crate::scheduled::check_update_cancelled(root)
            .is_err_and(|cancel_error| cancel_error == "UPDATE_CANCELLED")
        {
            return Err("UPDATE_CANCELLED".into());
        }
        return Err(error);
    }

    let message = uninstall_success_message(
        used_fallback,
        restore_gamefiles,
        remove_mods,
        reset_user_settings,
        purge_data,
    );
    crate::scheduled::write_status(root, "uninstalled", &message);
    log_uninstall(
        root,
        'I',
        &if purge_data {
            format!(
                "{message}. The recovery copy will be removed with the complete shroudforge folder"
            )
        } else {
            format!("{message}. Recoverable backup={}", backup_entry.display())
        },
    );
    Ok(())
}

fn ensure_uninstall_can_continue(
    root: &Path,
    restore_gamefiles: bool,
    remove_mods: bool,
    reset_user_settings: bool,
    purge_data: bool,
) -> Result<(), String> {
    crate::scheduled::check_update_cancelled(root)?;
    crate::update_queue_storage::update_update_queue(root, |queue| {
        if queue["run"]["state"] != "running" {
            return Err("the update queue stopped before uninstallation could begin".into());
        }
        let active_item_id = queue["run"]["activeItemId"].as_str().map(str::to_owned);
        queue["run"]["installing"] = serde_json::Value::Bool(true);
        queue["run"]["cancelable"] = serde_json::Value::Bool(!restore_gamefiles);
        if let Some(active_item_id) = active_item_id
            && let Some(item) = queue["items"].as_array_mut().and_then(|items| {
                items
                    .iter_mut()
                    .find(|item| item["id"].as_str() == Some(active_item_id.as_str()))
            })
        {
            item["state"] = serde_json::Value::String("installing".into());
            item["message"] = serde_json::Value::String(
                uninstall_action_message(
                    restore_gamefiles,
                    remove_mods,
                    reset_user_settings,
                    purge_data,
                )
                .into(),
            );
        }
        Ok(())
    })
}

fn set_uninstall_cancelable(root: &Path, cancelable: bool) -> Result<(), String> {
    crate::update_queue_storage::update_update_queue(root, |queue| {
        if queue["run"]["state"] != "running" || queue["run"]["installing"] != true {
            return Err("the update queue stopped during game file restoration".into());
        }
        queue["run"]["cancelable"] = serde_json::Value::Bool(cancelable);
        Ok(())
    })
}

fn program_paths(root: &Path, remove_mods: bool) -> Result<(Vec<String>, bool), String> {
    let manifest_path = shroudforge_loader_config::paths::version_file(root);
    let manifest = match fs::read(&manifest_path) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                log_uninstall(
                    root,
                    'W',
                    &format!(
                        "Installed version manifest is invalid. Using the known core-file list: {error}"
                    ),
                );
                None
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            log_uninstall(
                root,
                'W',
                "Installed version manifest is missing. Using the known core-file list",
            );
            None
        }
        Err(error) => {
            log_uninstall(
                root,
                'W',
                &format!(
                    "Could not read installed version manifest. Using the known core-file list: {error}"
                ),
            );
            None
        }
    };
    let (mut paths, used_fallback) = match manifest
        .as_ref()
        .and_then(|value| value["managedPaths"].as_array())
    {
        Some(entries) if !entries.is_empty() => {
            let mut paths = Vec::with_capacity(entries.len() + INJECTION_FILES.len());
            for entry in entries {
                let relative = entry
                    .as_str()
                    .ok_or("installed version has an invalid managed path")?;
                validate_release_path(relative)?;
                if !remove_mods && mod_relative_path(relative).is_some() {
                    continue;
                }
                paths.push(relative.to_owned());
            }
            (paths, false)
        }
        _ => {
            if manifest.is_some() {
                log_uninstall(
                    root,
                    'W',
                    "Installed version manifest has no managed files. Using the known core-file list",
                );
            }
            (
                FALLBACK_PROGRAM_FILES
                    .iter()
                    .map(|relative| (*relative).to_owned())
                    .collect(),
                true,
            )
        }
    };
    paths.extend(
        FALLBACK_PROGRAM_FILES
            .iter()
            .map(|relative| (*relative).to_owned()),
    );
    paths.extend(
        INJECTION_FILES
            .iter()
            .map(|relative| (*relative).to_owned()),
    );
    if remove_mods {
        collect_mod_paths(root, &mut paths)?;
    }
    paths.sort_by(|left, right| {
        (left == VERSION_MANIFEST)
            .cmp(&(right == VERSION_MANIFEST))
            .then_with(|| left.cmp(right))
    });
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    Ok((paths, used_fallback))
}

fn collect_mod_paths(root: &Path, paths: &mut Vec<String>) -> Result<(), String> {
    let mods = shroudforge_loader_config::paths::mods_dir(root);
    let metadata = match fs::symlink_metadata(&mods) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("could not inspect mods folder: {error}")),
    };
    validate_mod_entry(&mods, &metadata)?;
    if !metadata.is_dir() {
        return Err(format!("mods path is not a folder: {}", mods.display()));
    }
    let mods_root = mods
        .canonicalize()
        .map_err(|error| format!("could not resolve mods folder {}: {error}", mods.display()))?;
    let install_root = root
        .canonicalize()
        .map_err(|error| format!("could not resolve game folder {}: {error}", root.display()))?;
    if path_contains_path(&mods_root, &install_root) {
        return Err(format!(
            "configured mods folder overlaps the game installation. Change the Mods storage path before removing mods: {}",
            mods_root.display()
        ));
    }
    let data_directory = shroudforge_loader_config::paths::data_dir(root);
    match data_directory.canonicalize() {
        Ok(data_root) if path_contains_path(&data_root, &mods_root) => {
            return Err(format!(
                "configured mods folder is inside ShroudForge data. Change the Mods storage path before removing mods: {}",
                mods_root.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "could not validate the ShroudForge data folder before removing mods: {error}"
            ));
        }
    }
    collect_mod_paths_recursive(root, &mods_root, &mods_root, paths)
}

fn path_contains_path(parent: &Path, child: &Path) -> bool {
    let normalize = |path: &Path| {
        let mut value = path.to_string_lossy().replace('/', "\\").to_lowercase();
        while value.len() > 3 && value.ends_with('\\') {
            value.pop();
        }
        value
    };
    let parent = normalize(parent);
    let child = normalize(child);
    if parent == child {
        return true;
    }
    let prefix = if parent.ends_with('\\') {
        parent
    } else {
        format!("{parent}\\")
    };
    child.starts_with(&prefix)
}

fn collect_mod_paths_recursive(
    root: &Path,
    mods_root: &Path,
    directory: &Path,
    paths: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|error| {
        format!(
            "could not read mods folder {}: {error}",
            directory.display()
        )
    })? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        validate_mod_entry(&path, &metadata)?;
        if metadata.is_dir() {
            collect_mod_paths_recursive(root, mods_root, &path, paths)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(mods_root)
                .map_err(|error| error.to_string())?
                .to_str()
                .ok_or("mods folder contains a path that cannot be represented safely")?
                .replace('\\', "/");
            let managed = format!("mods/{relative}");
            validate_file_path(root, &managed)?;
            paths.push(managed);
        } else {
            return Err(format!(
                "unsupported file type in mods folder: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn validate_mod_entry(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    if metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0 {
        return Err(format!("reparse point in mods folder: {}", path.display()));
    }
    Ok(())
}

fn reset_loader_settings(root: &Path) -> Result<(), String> {
    shroudforge_loader_config::config::update_loader(root, |settings| {
        let paths = settings
            .get("paths")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        *settings = serde_json::json!({
            "schemaVersion": 1,
            "logging": {"minimumLevel": "INFO"},
            "modules": {},
            "paths": paths
        });
        Ok(())
    })
}

fn uninstall_action_message(
    restore_gamefiles: bool,
    remove_mods: bool,
    reset_user_settings: bool,
    purge_data: bool,
) -> &'static str {
    if purge_data {
        return if restore_gamefiles && remove_mods {
            "Restoring original game files, removing ShroudForge and selected mods, then purging all ShroudForge data"
        } else if restore_gamefiles {
            "Restoring original game files, removing ShroudForge, then purging all ShroudForge data"
        } else if remove_mods {
            "Removing ShroudForge files and selected mods, then purging all ShroudForge data"
        } else {
            "Removing ShroudForge files, then purging all ShroudForge data"
        };
    }
    if restore_gamefiles {
        return match (remove_mods, reset_user_settings) {
            (true, true) => {
                "Restoring original game files, removing ShroudForge and selected mods, and resetting user settings"
            }
            (true, false) => {
                "Restoring original game files before removing ShroudForge and selected mods"
            }
            (false, true) => {
                "Restoring original game files before removing ShroudForge and resetting user settings"
            }
            (false, false) => "Restoring original game files before removing ShroudForge",
        };
    }
    match (remove_mods, reset_user_settings) {
        (true, true) => "Removing ShroudForge files, selected mods and resetting user settings",
        (true, false) => "Removing ShroudForge files and selected mods",
        (false, true) => "Removing ShroudForge files and resetting user settings",
        (false, false) => "Removing ShroudForge program files",
    }
}

fn uninstall_success_message(
    used_fallback: bool,
    restore_gamefiles: bool,
    remove_mods: bool,
    reset_user_settings: bool,
    purge_data: bool,
) -> String {
    let program_files = if used_fallback {
        "Known ShroudForge core files were removed. Additional program files may remain because the installed file manifest could not be used."
    } else {
        "ShroudForge program files were removed."
    };
    let mods = if remove_mods && purge_data {
        "Mods were removed."
    } else if remove_mods {
        "Mods were removed and backed up in the recovery folder."
    } else {
        "Mods were kept."
    };
    let settings = if purge_data {
        "The complete shroudforge data folder, including settings, logs, updates and recovery files, is being removed recursively."
    } else if reset_user_settings {
        "ShroudForge settings were reset to defaults. Configured storage locations and other personal data were kept."
    } else {
        "ShroudForge settings and personal data were kept."
    };
    let gamefiles = if restore_gamefiles {
        "Original Enshrouded game files were restored and verified."
    } else {
        "Original Enshrouded game files were not restored."
    };
    format!("{gamefiles} {program_files} {mods} {settings}")
}

fn discard_unused_recovery_copy(root: &Path, backup_entry: &Path, error: String) -> String {
    match fs::remove_dir_all(backup_entry) {
        Ok(()) => error,
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => error,
        Err(cleanup_error) => {
            let detail = format!(
                "{error}. Could not remove the unused recovery copy at {}: {cleanup_error}",
                backup_entry.display()
            );
            log_uninstall(root, 'E', &detail);
            detail
        }
    }
}

fn remove_empty_program_directories(root: &Path, file: &Path) {
    let data_directory = shroudforge_loader_config::paths::data_dir(root);
    let mut directory = file.parent();
    while let Some(path) = directory {
        if path == data_directory || !path.starts_with(&data_directory) {
            break;
        }
        match fs::remove_dir(path) {
            Ok(()) => directory = path.parent(),
            Err(error)
                if error.kind() == std::io::ErrorKind::DirectoryNotEmpty
                    || error.kind() == std::io::ErrorKind::NotFound =>
            {
                break;
            }
            Err(_) => break,
        }
    }
}

fn log_uninstall(root: &Path, level: char, message: &str) {
    let _ = shroudforge_logging::append(root, level, "updater", message);
}

#[cfg(test)]
mod tests {
    use super::{mod_relative_path, program_paths, reset_loader_settings};
    use std::fs;

    #[test]
    fn uninstall_plan_never_includes_mod_paths() {
        let root = tempfile::tempdir().unwrap();
        let version_file = shroudforge_loader_config::paths::version_file(root.path());
        fs::create_dir_all(version_file.parent().unwrap()).unwrap();
        fs::write(
            version_file,
            br#"{"managedPaths":["mods/example-mod/mod.dll","Mods/another-mod/mod.dll","shroudforge/example.dll"]}"#,
        )
        .unwrap();

        let (paths, used_fallback) = program_paths(root.path(), false).unwrap();

        assert!(!used_fallback);
        assert!(paths.iter().all(|path| mod_relative_path(path).is_none()));
        assert!(paths.iter().any(|path| path == "dbghelp.dll"));
        assert!(paths.iter().any(|path| path == "dinput8.dll"));
        assert!(paths.iter().any(|path| path == "winmm.dll"));
        assert!(paths.iter().any(|path| path == "shroudforge/example.dll"));
    }

    #[test]
    fn opted_in_mod_cleanup_collects_mods_for_recoverable_removal() {
        let root = tempfile::tempdir().unwrap();
        let version_file = shroudforge_loader_config::paths::version_file(root.path());
        fs::create_dir_all(version_file.parent().unwrap()).unwrap();
        fs::write(
            version_file,
            br#"{"managedPaths":["shroudforge/example.dll"]}"#,
        )
        .unwrap();
        let mod_file = root.path().join("mods/example-mod/mod.dll");
        fs::create_dir_all(mod_file.parent().unwrap()).unwrap();
        fs::write(&mod_file, b"mod").unwrap();

        let (paths, used_fallback) = program_paths(root.path(), true).unwrap();

        assert!(!used_fallback);
        assert!(paths.iter().any(|path| path == "mods/example-mod/mod.dll"));
        assert!(mod_file.is_file());
    }

    #[test]
    fn resetting_user_settings_preserves_storage_paths() {
        let root = tempfile::tempdir().unwrap();
        shroudforge_loader_config::config::initialize_loader_config(root.path()).unwrap();
        shroudforge_loader_config::config::update_loader(root.path(), |settings| {
            settings["general"]["locale"] = serde_json::json!("fr");
            settings["paths"]["updates"] = serde_json::json!("custom-updates");
            settings["modules"]["updates"]["enabled"] = serde_json::Value::Bool(false);
            Ok(())
        })
        .unwrap();

        reset_loader_settings(root.path()).unwrap();
        let settings = shroudforge_loader_config::config::read_loader(root.path()).unwrap();

        assert_eq!(settings["general"]["locale"], "en-GB");
        assert_eq!(settings["paths"]["updates"], "custom-updates");
        assert_eq!(settings["modules"]["updates"]["enabled"], true);
    }
}
