#[cfg(windows)]
mod windows {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::SystemTime,
    };
    pub fn run() -> Result<(), String> {
        let result = run_install();
        if let Err(error) = &result {
            if let Ok(arguments) = Arguments::read() {
                if error == "UPDATE_CANCELLED" {
                    super::scheduled::write_status(
                        &arguments.root,
                        "cancelled",
                        "Update download cancelled; queue cleared",
                    );
                } else {
                    super::scheduled::write_status(&arguments.root, "error", error);
                    append_log(
                        &arguments.root,
                        'E',
                        &format!("Update worker failed before completion: {error}"),
                    );
                }
            }
        }
        result
    }

    pub(super) fn restore_gamefiles_worker(root: &Path) -> Result<(), String> {
        let status_path =
            shroudforge_package::paths::backups_dir(root).join("gamefiles-restore-status.json");
        let write_status = |status: &str, message: &str| {
            shroudforge_package::config::write_json(
                &status_path,
                &serde_json::json!({"schemaVersion":1,"status":status,"message":message,
                    "updatedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()}),
            )
        };
        let target = if root.join("enshrouded_server.exe").is_file() {
            "server"
        } else if root.join("enshrouded.exe").is_file() {
            "client"
        } else {
            return Err("could not identify the Enshrouded installation target".into());
        };
        write_status("waitingForGame", "Waiting for Enshrouded to close")?;
        append_log(
            root,
            'I',
            "Gamefile restoration is waiting for the game process to exit",
        );
        let result: Result<(), String> = (|| {
            super::scheduled::wait_for_game_processes(root)?;
            write_status("restoring", "Verifying and restoring original game files")?;
            let restored = shroudforge_package::backups::restore_originals(root, target)?;
            shroudforge_package::config::write_json(
                &status_path,
                &serde_json::json!({"schemaVersion":1,"status":"restart-required",
                    "message":"Original game files restored; restart Enshrouded",
                    "target":target,"result":restored,
                    "updatedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()}),
            )?;
            append_log(
                root,
                'I',
                "Original game files restored and verified; restart required",
            );
            Ok(())
        })();
        if let Err(error) = &result {
            let _ = write_status("error", error);
            append_log(root, 'E', &format!("Gamefile restoration failed: {error}"));
        }
        result
    }

    fn run_install() -> Result<(), String> {
        let arguments = Arguments::read()?;
        super::scheduled::write_status(
            &arguments.root,
            "waitingForGame",
            "Update is verified; waiting for Enshrouded to exit",
        );
        append_log(
            &arguments.root,
            'D',
            "Independent updater is waiting for Enshrouded processes to exit",
        );
        if let Err(error) = super::scheduled::wait_for_game_processes(&arguments.root) {
            append_log(
                &arguments.root,
                'E',
                &format!("Game process scan failed; update was not installed: {error}"),
            );
            return Err(error);
        }
        super::scheduled::check_update_cancelled(&arguments.root)?;
        super::scheduled::write_status(
            &arguments.root,
            "installing",
            "Validating the verified ShroudForge update",
        );
        validate_roots(&arguments.root, &arguments.staged)?;
        let source = arguments.staged.clone();
        if !source.join("shroudforge/version.json").is_file() {
            return Err("staged release does not contain shroudforge/version.json".into());
        }

        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let backup = shroudforge_package::paths::updates_dir(&arguments.root)
            .join("backups")
            .join(stamp.to_string());
        fs::create_dir_all(&backup).map_err(|error| error.to_string())?;
        super::scheduled::write_status(
            &arguments.root,
            "installing",
            "Preparing rollback backup before replacing files",
        );
        append_log(
            &arguments.root,
            'D',
            &format!("Starting update from {}", arguments.staged.display()),
        );

        let paths = managed_paths(&source)?;
        let obsolete_mod_paths = obsolete_managed_mod_paths(&arguments.root, &paths)?;
        let mut transaction_paths = paths.clone();
        transaction_paths.extend(obsolete_mod_paths.iter().cloned());
        transaction_paths.sort();
        transaction_paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        // Complete and verify backup before the first installed file changes.
        for relative in &transaction_paths {
            let is_incoming = paths
                .iter()
                .any(|incoming| incoming.eq_ignore_ascii_case(relative));
            if is_incoming {
                validate_file_path(&source, relative)?;
            }
            validate_file_path(&arguments.root, relative)?;
            let incoming = source.join(relative);
            if is_incoming && !incoming.is_file() {
                return Err(format!("missing release file: {relative}"));
            }
            let current = installed_path(&arguments.root, relative);
            if current.exists() {
                copy_entry(&current, &backup.join(relative))?;
            }
        }
        super::scheduled::write_status(
            &arguments.root,
            "installing",
            "Applying the verified update",
        );
        let result = apply(&source, &arguments.root, &paths).and_then(|()| {
            for relative in &obsolete_mod_paths {
                let current = installed_path(&arguments.root, relative);
                remove_entry(&current)?;
                remove_empty_mod_directories(&arguments.root, &current)?;
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                let release: serde_json::Value = serde_json::from_slice(
                    &fs::read(source.join("shroudforge/version.json"))
                        .map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let state = serde_json::json!({
                    "schemaVersion": 1, "status": "installed", "version": release["version"],
                    "build": release["build"], "installedAt": stamp, "backup": backup
                });
                if let Err(error) =
                    shroudforge_package::config::write_document(&arguments.root, "state", &state)
                {
                    append_log(
                        &arguments.root,
                        'E',
                        &format!("Installed files, but could not save update state: {error}"),
                    );
                }
                let _ = fs::remove_file(
                    shroudforge_package::paths::updates_dir(&arguments.root).join("pending.ready"),
                );
                let status = serde_json::json!({"schemaVersion":1,"operation":"systemStage","status":"installed","step":"complete","message":"System update installed successfully","version":release["version"],"updatedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
                let _ = shroudforge_package::config::write_json(
                    &shroudforge_package::paths::updates_dir(&arguments.root)
                        .join("updater-status.json"),
                    &status,
                );
                append_log(
                    &arguments.root,
                    'I',
                    &format!("Update installed; backup={}", backup.display()),
                );
                Ok(())
            }
            Err(error) => {
                append_log(
                    &arguments.root,
                    'E',
                    &format!("Update failed: {error}; restoring backup"),
                );
                if let Err(rollback_error) = restore(&backup, &arguments.root, &transaction_paths) {
                    append_log(
                        &arguments.root,
                        'E',
                        &format!("Rollback failed: {rollback_error}"),
                    );
                    return Err(format!("{error}; rollback also failed: {rollback_error}"));
                }
                Err(error)
            }
        }
    }

    struct Arguments {
        root: PathBuf,
        staged: PathBuf,
    }

    impl Arguments {
        fn read() -> Result<Self, String> {
            let values: Vec<String> = std::env::args().collect();
            let value = |name: &str| {
                values
                    .windows(2)
                    .find(|pair| pair[0] == name)
                    .map(|pair| pair[1].clone())
            };
            Ok(Self {
                root: PathBuf::from(value("--root").ok_or("missing --root")?),
                staged: PathBuf::from(value("--staged").ok_or("missing --staged")?),
            })
        }
    }

    fn validate_roots(root: &Path, staged: &Path) -> Result<(), String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("invalid install root: {error}"))?;
        let staged = staged
            .canonicalize()
            .map_err(|error| format!("invalid staged root: {error}"))?;
        let updates = shroudforge_package::paths::updates_dir(&root);
        if !staged.starts_with(&updates) {
            return Err("staged update is outside the configured updates folder".into());
        }
        if root == staged {
            return Err("staged update must not equal install root".into());
        }
        Ok(())
    }

    fn validate_relative_path(relative: &str) -> Result<PathBuf, String> {
        let path = Path::new(relative);
        if relative.is_empty()
            || relative.contains('\\')
            || relative.contains(':')
            || relative.contains('\0')
            || !path
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return Err(format!("unsafe managed path: {relative}"));
        }
        Ok(path.to_path_buf())
    }

    fn reserved_game_path(key: &str) -> bool {
        matches!(
            key,
            "enshrouded.exe" | "enshrouded_server.exe" | "enshrouded.log" | "steam_api64.dll"
        ) || key.starts_with("enshrouded_")
            || key.starts_with("enshrouded.kfc")
    }

    fn protected_user_path(key: &str) -> bool {
        let protected = [
            "shroudforge/config/modloader-config.json",
            "shroudforge/config/.shroudforge-write.lock",
            "shroudforge/state.json",
            "shroudforge/cache/",
            "shroudforge/exports/",
            "shroudforge/logs/",
            "shroudforge/ui/",
            "shroudforge/updates/",
            "shroudforge/runtime/heartbeat.json",
            "shroudforge/runtime/startup-assets.lock",
        ];
        protected.iter().any(|item| {
            if item.ends_with('/') {
                key.starts_with(item)
            } else {
                key == *item
            }
        })
    }

    fn validate_release_path(relative: &str) -> Result<PathBuf, String> {
        let path = validate_relative_path(relative)?;
        let key = relative.to_ascii_lowercase();
        if protected_user_path(&key) || reserved_game_path(&key) {
            return Err(format!(
                "release path is reserved for game or user data: {relative}"
            ));
        }
        Ok(path)
    }

    fn managed_paths(source: &Path) -> Result<Vec<String>, String> {
        let bytes = fs::read(source.join("shroudforge/version.json")).map_err(|e| e.to_string())?;
        let release: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let entries = release["managedPaths"]
            .as_array()
            .ok_or("release has no managedPaths")?;
        let mut paths = Vec::new();
        for entry in entries {
            let relative = entry.as_str().ok_or("invalid managed path")?;
            let path = validate_release_path(relative)?;
            // New releases list files, never whole mod/config directories.
            if !source.join(path).is_file() {
                return Err(format!("managed path is not a file: {relative}"));
            }
            paths.push(relative.to_owned());
        }
        paths.sort();
        paths.dedup();
        if !paths.iter().any(|p| p == "shroudforge/version.json") {
            return Err("release must manage shroudforge/version.json".into());
        }
        Ok(paths)
    }

    fn obsolete_managed_mod_paths(
        target: &Path,
        current: &[String],
    ) -> Result<Vec<String>, String> {
        let version_path = target.join("shroudforge/version.json");
        if !version_path.is_file() {
            return Ok(Vec::new());
        }
        let release: serde_json::Value =
            serde_json::from_slice(&fs::read(&version_path).map_err(|error| error.to_string())?)
                .map_err(|error| format!("installed version manifest is invalid: {error}"))?;
        let entries = release["managedPaths"]
            .as_array()
            .ok_or("installed version has no managedPaths")?;
        let mut obsolete = Vec::new();
        for entry in entries {
            let Some(relative) = entry.as_str() else {
                return Err("installed version has an invalid managed path".into());
            };
            if !relative.starts_with("mods/")
                || current
                    .iter()
                    .any(|path| path.eq_ignore_ascii_case(relative))
            {
                continue;
            }
            let mod_relative = relative.strip_prefix("mods/").unwrap_or_default();
            let path = Path::new(mod_relative);
            if mod_relative.is_empty()
                || relative.contains('\\')
                || relative.contains(':')
                || !path
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
            {
                return Err(format!("unsafe installed managed path: {relative}"));
            }
            obsolete.push(relative.to_owned());
        }
        obsolete.sort();
        obsolete.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        Ok(obsolete)
    }
    fn remove_empty_mod_directories(root: &Path, file: &Path) -> Result<(), String> {
        let mods = shroudforge_package::paths::mods_dir(root);
        let mut directory = file.parent();
        while let Some(path) = directory {
            if path == mods || !path.starts_with(&mods) {
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
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(())
    }

    fn installed_path(root: &Path, relative: &str) -> PathBuf {
        if let Some(mod_relative) = relative.strip_prefix("mods/") {
            shroudforge_package::paths::mods_dir(root).join(mod_relative)
        } else {
            root.join(relative)
        }
    }

    fn validate_file_path(root: &Path, relative: &str) -> Result<(), String> {
        use std::os::windows::fs::MetadataExt;
        let mut path = if relative.starts_with("mods/") {
            shroudforge_package::paths::mods_dir(root)
        } else {
            root.to_path_buf()
        };
        // Never follow a junction/symlink into another installation or user directory.
        let relative_path = relative.strip_prefix("mods/").unwrap_or(relative);
        for component in Path::new(relative_path).components() {
            path.push(component);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_attributes() & 0x400 != 0 => {
                    return Err(format!("reparse point in update path: {}", path.display()));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        if path.exists() && !path.is_file() {
            return Err(format!("update target is not a file: {}", path.display()));
        }
        Ok(())
    }

    fn apply(source: &Path, target: &Path, paths: &[String]) -> Result<(), String> {
        for relative in paths {
            let incoming = source.join(relative);
            if !incoming.exists() {
                return Err(format!("release is missing managed path: {relative}"));
            }
            let current = installed_path(target, relative);
            copy_entry(&incoming, &current)?;
        }
        Ok(())
    }

    fn restore(backup: &Path, target: &Path, paths: &[String]) -> Result<(), String> {
        for relative in paths {
            let saved = backup.join(relative);
            let current = installed_path(target, relative);
            remove_entry(&current)?;
            if saved.exists() {
                copy_entry(&saved, &current)?;
            }
        }
        Ok(())
    }

    fn copy_entry(source: &Path, target: &Path) -> Result<(), String> {
        if source.is_dir() {
            fs::create_dir_all(target).map_err(|error| error.to_string())?;
            for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
                let entry = entry.map_err(|error| error.to_string())?;
                copy_entry(&entry.path(), &target.join(entry.file_name()))?;
            }
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(source, target).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn remove_entry(path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        } else {
            fs::remove_file(path).map_err(|error| error.to_string())
        }
    }

    fn append_log(root: &Path, level: char, message: &str) {
        let _ = shroudforge_package::logging::append(root, level, "updater", message);
    }
}

#[cfg(windows)]
mod scheduled {
    use std::{
        fs,
        io::{Read, Write},
        path::{Path, PathBuf},
        process::Command,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, GetLastError},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
    };

    const MAX_UPDATE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const MAX_ARCHIVE_ENTRIES: usize = 4096;

    pub(super) fn write_status(root: &Path, status: &str, message: &str) {
        write_status_with_version(root, status, message, None);
    }

    pub(super) fn write_status_with_version(
        root: &Path,
        status: &str,
        message: &str,
        version: Option<&str>,
    ) {
        let previous_path =
            shroudforge_package::paths::updates_dir(root).join("updater-status.json");
        let previous = fs::read(&previous_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let keep_progress = status != "queued";
        let version = version.map(str::to_owned).or_else(|| {
            previous
                .as_ref()
                .and_then(|value| value["version"].as_str())
                .map(str::to_owned)
        });
        let downloaded = if keep_progress {
            previous
                .as_ref()
                .and_then(|value| value["downloadedBytes"].as_u64())
                .unwrap_or(0)
        } else {
            0
        };
        let total = if keep_progress {
            previous
                .as_ref()
                .and_then(|value| value.get("totalBytes"))
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };
        let speed = if keep_progress {
            previous
                .as_ref()
                .and_then(|value| value.get("bytesPerSecond"))
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };
        let value = serde_json::json!({"schemaVersion":1,"operation":"systemStage","status":status,"step":status,"message":message,"version":version,"downloadedBytes":downloaded,"totalBytes":total,"bytesPerSecond":speed,"updatedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
        let _ = shroudforge_package::config::write_json(
            &shroudforge_package::paths::updates_dir(root).join("updater-status.json"),
            &value,
        );
    }

    fn write_download_progress(
        root: &Path,
        version: &str,
        downloaded: u64,
        total: Option<u64>,
        speed: Option<u64>,
    ) {
        let value = serde_json::json!({"schemaVersion":1,"operation":"systemStage","status":"downloading","step":"download","message":"Downloading ShroudForge update","version":version,"downloadedBytes":downloaded,"totalBytes":total,"bytesPerSecond":speed,"updatedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
        let _ = shroudforge_package::config::write_json(
            &shroudforge_package::paths::updates_dir(root).join("updater-status.json"),
            &value,
        );
    }

    pub(super) fn stage_system_update(
        root: &Path,
        request: &serde_json::Value,
    ) -> Result<(), String> {
        let version = request["version"]
            .as_str()
            .ok_or("stage request has no version")?;
        let url = request["downloadUrl"]
            .as_str()
            .ok_or("stage request has no download URL")?;
        let checksum = request["checksum"]
            .as_str()
            .ok_or("stage request has no checksum")?
            .to_ascii_lowercase();
        let wait_pid = request["waitPid"].as_u64().unwrap_or(0) as u32;
        if !url.starts_with("https://")
            || checksum.len() != 64
            || !checksum.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("stage request URL or SHA-256 checksum is invalid".into());
        }
        let updates = shroudforge_package::paths::updates_dir(root);
        check_update_cancelled(root)?;
        fs::create_dir_all(&updates).map_err(|e| e.to_string())?;
        let download = updates.join("download.zip");
        let extraction = updates.join("pending-download");
        let pending = updates.join("pending");
        let _ = fs::remove_dir_all(&extraction);
        fs::create_dir_all(&extraction).map_err(|e| e.to_string())?;
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .user_agent(format!("ShroudForge/{version} updater"))
            .build()
            .map_err(|e| e.to_string())?;
        let mut response = client
            .get(url)
            .send()
            .map_err(|e| format!("download failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "release download returned HTTP {}",
                response.status()
            ));
        }
        let content_length = response.content_length();
        if content_length.is_some_and(|length| length > MAX_UPDATE_BYTES) {
            return Err("update package exceeds 2 GiB".into());
        }
        let mut file = fs::File::create(&download).map_err(|e| e.to_string())?;
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        let mut total = 0u64;
        let mut last_progress = std::time::Instant::now();
        let mut last_bytes = 0u64;
        write_download_progress(root, version, 0, content_length, Some(0));
        let mut buffer = [0u8; 1024 * 1024];
        loop {
            check_update_cancelled(root)?;
            let count = response.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            total = total.saturating_add(count as u64);
            if total > MAX_UPDATE_BYTES {
                return Err("update package exceeds 2 GiB".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|e| e.to_string())?;
            hasher.update(&buffer[..count]);
            if last_progress.elapsed() >= std::time::Duration::from_millis(300) {
                let elapsed = last_progress.elapsed().as_secs_f64().max(0.001);
                let speed = ((total - last_bytes) as f64 / elapsed) as u64;
                write_download_progress(root, version, total, content_length, Some(speed));
                last_progress = std::time::Instant::now();
                last_bytes = total;
            }
        }
        file.flush().map_err(|e| e.to_string())?;
        write_download_progress(root, version, total, content_length, Some(0));
        write_status(
            root,
            "verifying",
            "Verifying the downloaded package checksum",
        );
        if format!("{:x}", hasher.finalize()) != checksum {
            return Err("update package SHA-256 checksum does not match".into());
        }
        write_status(root, "extracting", "Extracting and validating update files");
        let source = fs::File::open(&download).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(source).map_err(|e| e.to_string())?;
        if archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err("update archive contains too many entries".into());
        }
        let mut expanded = 0u64;
        for index in 0..archive.len() {
            check_update_cancelled(root)?;
            let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
            let relative = entry
                .enclosed_name()
                .ok_or_else(|| format!("unsafe archive path: {}", entry.name()))?
                .to_path_buf();
            if relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err(format!("unsafe archive path: {}", entry.name()));
            }
            expanded = expanded.saturating_add(entry.size());
            if expanded > 8 * MAX_UPDATE_BYTES {
                return Err("expanded update archive exceeds 16 GiB".into());
            }
            let output = extraction.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(&output).map_err(|e| e.to_string())?;
            } else {
                if let Some(parent) = output.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut file = fs::File::create(&output).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
                file.flush().map_err(|e| e.to_string())?;
            }
        }
        for required in [
            "shroudforge/version.json",
            "shroudforge/shroudforge.exe",
            "shroudforge/shroudforge-updater.exe",
        ] {
            if !extraction.join(required).is_file() {
                return Err(format!("update package is missing {required}"));
            }
        }
        let _ = fs::remove_file(updates.join("pending.ready"));
        if pending.exists() {
            fs::remove_dir_all(&pending).map_err(|e| e.to_string())?;
        }
        fs::rename(&extraction, &pending)
            .map_err(|e| format!("could not promote verified update: {e}"))?;
        let ready = serde_json::json!({"version":version,"checksumAlgorithm":"SHA-256","checksum":checksum,"stagedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
        shroudforge_package::config::write_json(&updates.join("pending.ready"), &ready)?;
        let _ = fs::remove_file(download);
        let queue =
            serde_json::json!({"schemaVersion":1,"operation":"installPending","waitPid":wait_pid});
        shroudforge_package::config::write_json(&updates.join("worker-queue.json"), &queue)?;
        Ok(())
    }

    fn cancel_path(root: &Path) -> PathBuf {
        shroudforge_package::paths::updates_dir(root).join("cancel-system-update")
    }

    pub(super) fn check_update_cancelled(root: &Path) -> Result<(), String> {
        if cancel_path(root).exists() {
            Err("UPDATE_CANCELLED".into())
        } else {
            Ok(())
        }
    }

    pub(super) fn clear_system_update(root: &Path) -> Result<(), String> {
        let updates = shroudforge_package::paths::updates_dir(root);
        fs::create_dir_all(&updates).map_err(|error| error.to_string())?;
        fs::write(cancel_path(root), b"cancel").map_err(|error| error.to_string())?;
        for path in [
            updates.join("system-stage-request.json"),
            updates.join("worker-queue.json"),
            updates.join("pending.ready"),
            updates.join("download.zip"),
        ] {
            let _ = fs::remove_file(path);
        }
        for path in [updates.join("pending-download"), updates.join("pending")] {
            if path.exists() {
                let _ = fs::remove_dir_all(path);
            }
        }
        write_status(
            root,
            "cancelled",
            "Update download cancelled; queue cleared",
        );
        Ok(())
    }

    pub(super) fn wait_for_game_processes(root: &Path) -> Result<(), String> {
        use std::{mem::size_of, time::Duration};
        let root = root
            .canonicalize()
            .map_err(|e| format!("invalid install root for process scan: {e}"))?;
        let root_key = root.to_string_lossy().to_lowercase();
        let mut last_waiting_pid = None;
        loop {
            check_update_cancelled(&root)?;
            let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
            if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                return Err(format!(
                    "could not enumerate game processes (Windows error {})",
                    unsafe { GetLastError() }
                ));
            }
            let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut running: Option<(u32, String)> = None;
            let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
            while has_entry {
                let exe = String::from_utf16_lossy(
                    &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(entry.szExeFile.len())],
                );
                if exe.eq_ignore_ascii_case("enshrouded.exe")
                    || exe.eq_ignore_ascii_case("enshrouded_server.exe")
                {
                    let process = unsafe {
                        OpenProcess(
                            PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000,
                            0,
                            entry.th32ProcessID,
                        )
                    };
                    if process.is_null() {
                        let error = unsafe { GetLastError() };
                        if error != ERROR_INVALID_PARAMETER {
                            unsafe { CloseHandle(snapshot) };
                            return Err(format!(
                                "cannot verify Enshrouded process {} (Windows error {error})",
                                entry.th32ProcessID
                            ));
                        }
                    } else {
                        let mut path = vec![0u16; 32768];
                        let mut length = path.len() as u32;
                        let queried = unsafe {
                            QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length)
                        } != 0;
                        unsafe { CloseHandle(process) };
                        if !queried {
                            unsafe { CloseHandle(snapshot) };
                            return Err(format!(
                                "cannot read image path for Enshrouded process {}",
                                entry.th32ProcessID
                            ));
                        }
                        let image_path = String::from_utf16_lossy(&path[..length as usize]);
                        let image_root = PathBuf::from(&image_path)
                            .parent()
                            .and_then(|path| path.canonicalize().ok());
                        // Canonicalize both sides so Steam libraries reached through junctions
                        // still match. If the executable path cannot be resolved, wait safely
                        // instead of assuming the game has exited.
                        let belongs_to_install = image_root
                            .map(|path| path.to_string_lossy().eq_ignore_ascii_case(&root_key))
                            .unwrap_or(true);
                        if belongs_to_install {
                            running = Some((entry.th32ProcessID, image_path));
                            break;
                        }
                    }
                }
                has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
            }
            unsafe { CloseHandle(snapshot) };
            if let Some((pid, image_path)) = running {
                if last_waiting_pid != Some(pid) {
                    let _ = shroudforge_package::logging::append(
                        &root,
                        'D',
                        "updater",
                        &format!(
                            "Enshrouded is still running (PID {pid}, {image_path}); waiting before installation"
                        ),
                    );
                    last_waiting_pid = Some(pid);
                }
            } else {
                if last_waiting_pid.is_some() {
                    let _ = shroudforge_package::logging::append(
                        &root,
                        'I',
                        "updater",
                        "Enshrouded process exited; updater continues",
                    );
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    fn task_id(root: &Path) -> Result<String, String> {
        let canonical = root.canonicalize().map_err(|e| e.to_string())?;
        let mut hash = 0xcbf29ce484222325u64;
        for byte in canonical.to_string_lossy().to_ascii_lowercase().bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Ok(format!("ShroudForgeUpdater_{hash:016x}"))
    }

    fn task_executable(root: &Path) -> Result<PathBuf, String> {
        let source = shroudforge_package::paths::updater_executable(root);
        if !source.is_file() {
            return Err(format!("updater executable missing: {}", source.display()));
        }
        let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
        let modified = fs::metadata(&source)
            .and_then(|m| m.modified())
            .map_err(|e| e.to_string())?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let destination = PathBuf::from(local)
            .join("ShroudForge")
            .join("Updater")
            .join(task_id(root)?)
            .join(modified.to_string())
            .join("shroudforge-updater.exe");
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if !destination.is_file() {
            fs::copy(&source, &destination)
                .map_err(|e| format!("cannot install independent updater: {e}"))?;
        }
        Ok(destination)
    }

    pub fn open_update_window(root: &Path) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let source = shroudforge_package::paths::loader_executable(root);
        if !source.is_file() {
            return Err(format!(
                "ShroudForge UI executable is missing: {}",
                source.display()
            ));
        }
        let updater = task_executable(root)?;
        let directory = updater
            .parent()
            .ok_or("updater cache has no parent directory")?;
        let metadata = fs::metadata(&source).map_err(|error| error.to_string())?;
        let changed = metadata
            .modified()
            .map_err(|error| error.to_string())?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let ui = directory.join(format!("shroudforge-ui-{changed}.exe"));
        if !ui.is_file() {
            fs::copy(&source, &ui)
                .map_err(|error| format!("could not prepare independent update window: {error}"))?;
        }
        let target = if root.join("enshrouded.exe").is_file() {
            "client"
        } else {
            "server"
        };
        Command::new(&ui)
            .args(["--module-ui", "--desktop", "--updater-window", "--root"])
            .arg(root)
            .args(["--target", target])
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|error| format!("could not launch independent update window: {error}"))?;
        Ok(())
    }

    pub fn start_system_waiter(root: &Path) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let executable = task_executable(root)?;
        let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
        Command::new(executable)
            .args(["--run-queue", "--root"])
            .arg(canonical_root)
            .creation_flags(0x08000000)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("could not start the queued update waiter: {error}"))
    }

    fn ensure_task(root: &Path) -> Result<String, String> {
        let executable = task_executable(root)?;
        let name = task_id(root)?;
        let action = format!(
            "\"{}\" --run-queue --root \"{}\"",
            executable.display(),
            root.canonicalize().map_err(|e| e.to_string())?.display()
        );
        let output = Command::new("schtasks.exe")
            .args([
                "/Create", "/SC", "ONLOGON", "/TN", &name, "/TR", &action, "/F", "/RL", "LIMITED",
                "/IT",
            ])
            .output()
            .map_err(|e| format!("could not register updater task: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "could not register updater task: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(name)
    }

    fn spawn_worker(root: &Path) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let executable = task_executable(root)?;
        let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
        Command::new(executable)
            .args(["--run-queue", "--root"])
            .arg(canonical_root)
            .creation_flags(0x08000000)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("could not start updater worker: {error}"))
    }

    pub fn request(root: &Path) -> Result<(), String> {
        let name = match ensure_task(root) {
            Ok(name) => name,
            Err(task_error) => {
                return spawn_worker(root).map_err(|spawn_error| {
                    format!("{task_error}; direct updater start also failed: {spawn_error}")
                });
            }
        };
        let output = match Command::new("schtasks.exe")
            .args(["/Run", "/TN", &name])
            .output()
        {
            Ok(output) => output,
            Err(task_error) => {
                let task_error = format!("could not start scheduled updater: {task_error}");
                return spawn_worker(root).map_err(|spawn_error| {
                    format!("{task_error}; direct updater start also failed: {spawn_error}")
                });
            }
        };
        if !output.status.success() {
            let task_error = format!(
                "could not start scheduled updater: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return spawn_worker(root).map_err(|spawn_error| {
                format!("{task_error}; direct updater start also failed: {spawn_error}")
            });
        }
        Ok(())
    }
}

#[cfg(windows)]
pub fn request_worker(root: &std::path::Path) -> Result<(), String> {
    scheduled::request(root)
}

#[cfg(not(windows))]
pub fn request_worker(_: &std::path::Path) -> Result<(), String> {
    Err("scheduled updater is available on Windows only".into())
}

#[cfg(windows)]
pub fn request_install_after_game(root: &std::path::Path, pid: u32) -> Result<(), String> {
    let request = serde_json::json!({"schemaVersion":1,"operation":"installPending","waitPid":pid});
    shroudforge_package::config::write_json(
        &shroudforge_package::paths::updates_dir(root).join("worker-queue.json"),
        &request,
    )
    .map_err(|e| e.to_string())?;
    request_worker(root)
}

#[cfg(windows)]
pub fn request_mod_install(
    root: &std::path::Path,
    project_id: &str,
    title: &str,
    provider: serde_json::Value,
) -> Result<(), String> {
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("invalid mod project ID".into());
    }
    enqueue_update_item(
        root,
        serde_json::json!({"id":format!("mod-install:{project_id}"),"kind":"mod","operation":"install","title":title,"version":"","state":"queued","selected":true,"payload":{"projectId":project_id,"provider":provider}}),
    )
}

#[cfg(windows)]
pub fn request_mod_update(
    root: &std::path::Path,
    project_id: &str,
    mod_id: &str,
    title: &str,
    provider: serde_json::Value,
) -> Result<(), String> {
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        || mod_id.is_empty()
        || mod_id.len() > 80
        || !mod_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("invalid mod project or package ID".into());
    }
    enqueue_update_item(
        root,
        serde_json::json!({"id":format!("mod-update:{project_id}:{mod_id}"),"kind":"mod","operation":"update","title":title,"version":"","state":"queued","selected":true,"payload":{"projectId":project_id,"modId":mod_id,"provider":provider}}),
    )
}

#[cfg(not(windows))]
pub fn request_mod_update(
    _: &std::path::Path,
    _: &str,
    _: &str,
    _: &str,
    _: serde_json::Value,
) -> Result<(), String> {
    Err("scheduled updater is available on Windows only".into())
}

#[cfg(windows)]
pub fn request_runtime_mod_reload(
    root: &std::path::Path,
    mod_ids: &[String],
) -> Result<(), String> {
    request_runtime_mod_action(root, "reload", mod_ids)
}

#[cfg(windows)]
pub fn request_runtime_mod_unload(
    root: &std::path::Path,
    mod_ids: &[String],
) -> Result<(), String> {
    request_runtime_mod_action(root, "unload", mod_ids)
}

#[cfg(windows)]
fn request_runtime_mod_action(
    root: &std::path::Path,
    operation: &str,
    mod_ids: &[String],
) -> Result<(), String> {
    let request_id = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let request_path =
        shroudforge_package::paths::runtime_dir(root).join("mod-reload-request.json");
    let result_path = shroudforge_package::paths::runtime_dir(root).join("mod-reload-result.json");
    let request =
        serde_json::json!({"requestId":request_id,"operation":operation,"modIds":mod_ids});
    shroudforge_package::config::write_json(&request_path, &request)
        .map_err(|error| format!("could not request live mod reload: {error}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Ok(bytes) = std::fs::read(&result_path) {
            if let Ok(result) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if result["requestId"].as_str() == Some(&request_id) {
                    let _ = std::fs::remove_file(&request_path);
                    let expected_status = if operation == "unload" {
                        "unloaded"
                    } else {
                        "reloaded"
                    };
                    return if result["status"] == expected_status {
                        Ok(())
                    } else {
                        Err(result["message"]
                            .as_str()
                            .unwrap_or("live runtime mod reload failed")
                            .to_owned())
                    };
                }
            }
        }
        if std::time::Instant::now() >= deadline {
            let _ = std::fs::remove_file(&request_path);
            return Err("timed out waiting for the running game to reload its mod runtime".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(not(windows))]
pub fn request_runtime_mod_reload(_: &std::path::Path, _: &[String]) -> Result<(), String> {
    Err("live runtime mod reload is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn request_runtime_mod_unload(_: &std::path::Path, _: &[String]) -> Result<(), String> {
    Err("live runtime mod reload is available on Windows only".into())
}

fn update_queue_path(root: &std::path::Path) -> std::path::PathBuf {
    shroudforge_package::paths::updates_dir(root).join("update-queue.json")
}

fn read_update_queue_value(root: &std::path::Path) -> serde_json::Value {
    std::fs::read(update_queue_path(root))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .filter(|value: &serde_json::Value| value["items"].is_array())
        .unwrap_or_else(|| serde_json::json!({"schemaVersion":1,"items":[],"run":null}))
}

#[cfg(windows)]
#[derive(Debug)]
struct HeadlessControlAction {
    name: String,
    request_id: String,
    parameters: serde_json::Value,
}

/// Start a small in-process controller for commands written to the loader
/// configuration. The game process owns this thread, while actual downloads
/// and file replacement continue to run in the existing updater worker.
#[cfg(windows)]
pub fn start_headless_control_worker(root: &std::path::Path) -> Result<(), String> {
    use std::sync::{Mutex, OnceLock};

    static STARTED_ROOTS: OnceLock<Mutex<std::collections::HashSet<std::path::PathBuf>>> =
        OnceLock::new();
    let root = root
        .canonicalize()
        .map_err(|error| format!("could not resolve game root for config controller: {error}"))?;
    let started = STARTED_ROOTS.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
    {
        let mut roots = started
            .lock()
            .map_err(|_| "headless config controller registry is unavailable")?;
        if !roots.insert(root.clone()) {
            return Ok(());
        }
    }

    std::thread::Builder::new()
        .name("shroudforge-config-controller".into())
        .spawn(move || {
            let mut next_auto_update_check = std::time::Instant::now();
            loop {
                if std::time::Instant::now() >= next_auto_update_check {
                    let settings = shroudforge_package::config::read_loader(&root).ok();
                    let enabled = settings
                        .as_ref()
                        .and_then(|value| value.pointer("/modules/updates/system/enabled"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true);
                    let interval_minutes = settings
                        .as_ref()
                        .and_then(|value| value.pointer("/modules/updates/system/checkMinutes"))
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(60)
                        .clamp(5, 1440);
                    let last_check_at = settings
                        .as_ref()
                        .and_then(|value| value.pointer("/modules/updates/system/lastCheckAt"))
                        .and_then(serde_json::Value::as_u64);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let interval_seconds = interval_minutes * 60;
                    let seconds_until_check = last_check_at
                        .map(|checked| interval_seconds.saturating_sub(now.saturating_sub(checked)))
                        .unwrap_or(0);
                    if enabled && seconds_until_check == 0 {
                        match headless_check_system_updates(&root) {
                            Ok(message) => {
                                let _ = shroudforge_package::logging::append(
                                    &root,
                                    'I',
                                    "updates",
                                    &format!("Headless update check: {message}"),
                                );
                            }
                            Err(error) => {
                                let _ = shroudforge_package::logging::append(
                                    &root,
                                    'W',
                                    "updates",
                                    &format!("Headless update check failed: {error}"),
                                );
                            }
                        }
                    }
                    next_auto_update_check = std::time::Instant::now()
                        + std::time::Duration::from_secs(if enabled {
                            if seconds_until_check == 0 {
                                interval_seconds
                            } else {
                                seconds_until_check
                            }
                        } else {
                            60
                        });
                }
                match claim_headless_control_actions(&root) {
                    Ok(actions) => {
                        for action in actions {
                            let result = execute_headless_control_action(&root, &action);
                            let (state, message) = match result {
                                Ok(message) => ("succeeded", message),
                                Err(error) => ("failed", error),
                            };
                            mark_headless_control_action(
                                &root,
                                &action.name,
                                &action.request_id,
                                state,
                                &message,
                            );
                        }
                    }
                    Err(error) => {
                        let _ = shroudforge_package::logging::append(
                            &root,
                            'E',
                            "config-controller",
                            &format!("Could not read pending server actions: {error}"),
                        );
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        })
        .map(|_| ())
        .map_err(|error| format!("could not start headless config controller: {error}"))
}

#[cfg(not(windows))]
pub fn start_headless_control_worker(_: &std::path::Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn claim_headless_control_actions(
    root: &std::path::Path,
) -> Result<Vec<HeadlessControlAction>, String> {
    const SUPPORTED: &[&str] = &[
        "checkUpdates",
        "installMod",
        "updateMod",
        "queueSystemUpdate",
        "selectUpdateQueueItems",
        "removeUpdateQueueItem",
        "clearUpdateQueue",
        "cancelUpdate",
        "startUpdateQueue",
        "stageUpdate",
        "restoreGamefiles",
        "captureGamefileOriginals",
    ];
    let current = shroudforge_package::config::read_loader(root)?;
    if !SUPPORTED.iter().any(|name| {
        current
            .pointer(&format!("/control/actions/{name}"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    }) {
        return Ok(Vec::new());
    }
    let mut claimed = Vec::new();
    shroudforge_package::config::update_loader(root, |config| {
        // Queue additions run before startUpdateQueue, so a config can request
        // an install/update and start the resulting selected item in one save.
        for name in SUPPORTED {
            if config
                .pointer(&format!("/control/actions/{name}"))
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                continue;
            }
            let request_id = format!(
                "headless-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            let parameters = config
                .pointer(&format!("/control/parameters/{name}"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            config["control"]["actions"][*name] = serde_json::Value::Bool(false);
            config["control"]["results"][*name] = serde_json::json!({
                "requestId": request_id,
                "state": "running",
                "result": "Accepted by headless update controller"
            });
            claimed.push(HeadlessControlAction {
                name: (*name).into(),
                request_id,
                parameters,
            });
        }
        Ok(())
    })?;
    Ok(claimed)
}

#[cfg(windows)]
fn execute_headless_control_action(
    root: &std::path::Path,
    action: &HeadlessControlAction,
) -> Result<String, String> {
    let parameters = &action.parameters;
    let ids = || -> Result<Vec<String>, String> {
        parameters
            .get("ids")
            .and_then(serde_json::Value::as_array)
            .ok_or("control action parameter 'ids' must be an array")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or("control action item IDs must be strings".into())
            })
            .collect()
    };
    let provider = || -> Result<serde_json::Value, String> {
        let provider =
            shroudforge_package::config::read_loader(root)?["catalog"]["provider"].clone();
        if !provider.is_object()
            || provider["enabled"].as_bool() != Some(true)
            || provider["kind"].as_str() != Some("shroudedit")
        {
            return Err("ShroudEdit catalog access is not enabled in modloader-config.json".into());
        }
        Ok(provider)
    };
    match action.name.as_str() {
        "checkUpdates" => headless_check_system_updates(root),
        "queueSystemUpdate" => {
            let release = shroudforge_package::config::read_loader(root)?
                .pointer("/modules/updates/system/latestRelease")
                .cloned()
                .filter(serde_json::Value::is_object)
                .ok_or("run checkUpdates first; no verified release is stored in modloader-config.json")?;
            if release["updateAvailable"].as_bool() != Some(true) {
                return Err("the stored release is not newer than the installed version".into());
            }
            enqueue_system_update(root, release)?;
            Ok("System update added to the shared update queue".into())
        }
        "stageUpdate" => {
            let config = shroudforge_package::config::read_loader(root)?;
            let mut release = config
                .pointer("/modules/updates/system/latestRelease")
                .cloned()
                .filter(serde_json::Value::is_object);
            if release.is_none() {
                headless_check_system_updates(root)?;
                release = shroudforge_package::config::read_loader(root)?
                    .pointer("/modules/updates/system/latestRelease")
                    .cloned()
                    .filter(serde_json::Value::is_object);
            }
            let release = release.ok_or("no valid ShroudForge release is available")?;
            if release["updateAvailable"].as_bool() != Some(true) {
                return Err("ShroudForge is already up to date".into());
            }
            let version = release["version"]
                .as_str()
                .ok_or("stored release has no version")?
                .to_owned();
            enqueue_system_update(root, release)?;
            let wait_for_game = parameters
                .get("waitForGame")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            start_update_queue(root, &[format!("system:{version}")], wait_for_game, false)?;
            Ok("Headless system update queued; updater will wait for Enshrouded to stop".into())
        }
        "selectUpdateQueueItems" => {
            let selected = parameters
                .get("selected")
                .and_then(serde_json::Value::as_bool)
                .ok_or("control action parameter 'selected' must be a boolean")?;
            select_update_queue_items(root, &ids()?, selected)?;
            Ok("Update queue selection saved".into())
        }
        "removeUpdateQueueItem" => {
            let key = parameters
                .get("key")
                .and_then(serde_json::Value::as_str)
                .ok_or("control action parameter 'key' is required")?;
            remove_update_queue_item(root, key)?;
            Ok("Update removed from the shared queue".into())
        }
        "startUpdateQueue" => {
            let mut selected_ids = ids()?;
            if selected_ids.is_empty() {
                selected_ids = read_update_queue_value(root)["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|item| item["selected"].as_bool() == Some(true))
                    .filter_map(|item| item["id"].as_str().map(str::to_owned))
                    .collect();
            }
            let wait_for_game = parameters
                .get("waitForGame")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            start_update_queue(root, &selected_ids, wait_for_game, false)?;
            Ok("Headless update worker started; progress is in the shared update queue".into())
        }
        "clearUpdateQueue" => {
            clear_update_queue(root)?;
            Ok("Update queue cleared".into())
        }
        "cancelUpdate" => {
            cancel_update_queue(root)?;
            Ok("Update queue cancellation requested".into())
        }
        "restoreGamefiles" => {
            request_gamefiles_restore(root)?;
            Ok("Gamefile restoration was handed to the updater".into())
        }
        "captureGamefileOriginals" => {
            let target = if root.join("enshrouded_server.exe").is_file() {
                "server"
            } else if root.join("enshrouded.exe").is_file() {
                "client"
            } else {
                return Err("could not identify the Enshrouded installation target".into());
            };
            let status = shroudforge_package::backups::ensure_originals(root, target)?;
            Ok(format!(
                "Original gamefile backup status: {}",
                status["status"].as_str().unwrap_or("unknown")
            ))
        }
        "installMod" => {
            let project_id = parameters
                .get("projectId")
                .and_then(serde_json::Value::as_str)
                .ok_or("control action parameter 'projectId' is required")?;
            let title = parameters
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(project_id);
            request_mod_install(root, project_id, title, provider()?)?;
            Ok(format!(
                "Mod installation for {project_id} added to the shared update queue"
            ))
        }
        "updateMod" => {
            let mod_id = parameters
                .get("modId")
                .and_then(serde_json::Value::as_str)
                .ok_or("control action parameter 'modId' is required")?;
            let catalog = shroudforge_package::config::read_document(root, "catalog-state")?;
            let project_id = catalog
                .get("projects")
                .and_then(serde_json::Value::as_object)
                .and_then(|projects| {
                    projects.iter().find_map(|(project_id, item)| {
                        (item.get("modId").and_then(serde_json::Value::as_str) == Some(mod_id))
                            .then(|| project_id.clone())
                    })
                })
                .ok_or("this mod is not registered as a ShroudEdit installation")?;
            let title = parameters
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(mod_id);
            request_mod_update(root, &project_id, mod_id, title, provider()?)?;
            Ok(format!(
                "Mod update for {mod_id} added to the shared update queue"
            ))
        }
        _ => Err(format!(
            "unsupported headless control action '{}': use the Modloader UI",
            action.name
        )),
    }
}

#[cfg(windows)]
fn headless_check_system_updates(root: &std::path::Path) -> Result<String, String> {
    use semver::Version;

    let config = shroudforge_package::config::read_loader(root)?;
    if config
        .pointer("/modules/updates/system/enabled")
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        return Err("system update checks are disabled in modloader-config.json".into());
    }
    let include_prereleases = config
        .pointer("/modules/updates/system/includePrereleases")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let current = std::fs::read(shroudforge_package::paths::version_file(root))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let current_version = current
        .as_ref()
        .and_then(|value| value["version"].as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let current_build = current
        .as_ref()
        .and_then(|value| value["build"].as_u64())
        .or_else(|| {
            current
                .as_ref()
                .and_then(|value| value["build"].as_str()?.parse().ok())
        })
        .unwrap_or(0);

    #[derive(serde::Deserialize)]
    struct GithubRelease {
        tag_name: String,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        prerelease: bool,
        #[serde(default)]
        body: String,
        html_url: String,
        #[serde(default)]
        assets: Vec<GithubAsset>,
    }
    #[derive(serde::Deserialize)]
    struct GithubAsset {
        name: String,
        browser_download_url: String,
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(format!("ShroudForge/{current_version} updater"))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get("https://api.github.com/repos/bonsaibauer/shroudforge/releases?per_page=100")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|error| format!("GitHub is unreachable: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        let checked_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        shroudforge_package::config::update_loader(root, |value| {
            value["modules"]["updates"]["system"]["latestRelease"] = serde_json::Value::Null;
            value["modules"]["updates"]["system"]["lastCheckAt"] = serde_json::json!(checked_at);
            Ok(())
        })?;
        return Ok("No ShroudForge release has been published yet".into());
    }
    if !response.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", response.status()));
    }
    let releases: Vec<GithubRelease> = response
        .json()
        .map_err(|error| format!("Invalid GitHub response: {error}"))?;
    let mut eligible = releases
        .into_iter()
        .filter(|release| !release.draft && (include_prereleases || !release.prerelease))
        .filter_map(|release| {
            let version = release.tag_name.trim_start_matches('v').to_owned();
            let (base_version, build) = version
                .rsplit_once("-build.")
                .or_else(|| version.rsplit_once("-dev."))?;
            let parsed = Version::parse(base_version).ok()?;
            let build = build.parse::<u64>().ok()?;
            let archive = release.assets.iter().find(|asset| {
                asset.name.starts_with("shroudforge-") && asset.name.ends_with(".zip")
            })?;
            let checksum_name = format!("{}.sha256", archive.name);
            let checksum = release
                .assets
                .iter()
                .find(|asset| asset.name == checksum_name)?;
            let base_version = base_version.to_owned();
            let download_url = archive.browser_download_url.clone();
            let checksum_url = checksum.browser_download_url.clone();
            Some((
                parsed,
                build,
                version,
                base_version,
                release,
                download_url,
                checksum_url,
            ))
        })
        .collect::<Vec<_>>();
    eligible.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let Some((_, build, version, base_version, release, download_url, checksum_url)) =
        eligible.pop()
    else {
        let checked_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        shroudforge_package::config::update_loader(root, |value| {
            value["modules"]["updates"]["system"]["latestRelease"] = serde_json::Value::Null;
            value["modules"]["updates"]["system"]["lastCheckAt"] = serde_json::json!(checked_at);
            Ok(())
        })?;
        return Ok("No compatible ShroudForge release was found".into());
    };
    let checksum_response = client
        .get(checksum_url)
        .send()
        .map_err(|error| format!("Could not download GitHub checksum: {error}"))?;
    if !checksum_response.status().is_success() {
        return Err(format!(
            "GitHub checksum endpoint returned HTTP {}",
            checksum_response.status()
        ));
    }
    let checksum = checksum_response
        .text()
        .map_err(|error| format!("Could not read GitHub checksum: {error}"))?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("GitHub release does not contain a valid SHA-256 checksum".into());
    }
    let update_available = Version::parse(current_version.trim_start_matches('v'))
        .ok()
        .zip(Version::parse(&base_version).ok())
        .is_some_and(|(current, latest)| {
            latest > current || (latest == current && build > current_build)
        });
    let message = if release.body.trim().is_empty() {
        "A new ShroudForge version is available on GitHub.".to_owned()
    } else {
        release.body.chars().take(20_000).collect()
    };
    let checked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    shroudforge_package::config::update_loader(root, |value| {
        value["modules"]["updates"]["system"]["lastCheckAt"] = serde_json::json!(checked_at);
        value["modules"]["updates"]["system"]["latestRelease"] = serde_json::json!({
            "version": version.clone(),
            "baseVersion": base_version.clone(),
            "build": build,
            "downloadUrl": download_url.clone(),
            "checksum": checksum.clone(),
            "releaseUrl": release.html_url.clone(),
            "message": message.clone(),
            "prerelease": release.prerelease,
            "updateAvailable": update_available,
            "checkedAt": checked_at
        });
        Ok(())
    })?;
    Ok(if update_available {
        format!("ShroudForge update {version} is available")
    } else {
        "ShroudForge is up to date".into()
    })
}

#[cfg(windows)]
fn mark_headless_control_action(
    root: &std::path::Path,
    name: &str,
    request_id: &str,
    state: &str,
    message: &str,
) {
    let _ = shroudforge_package::config::update_loader(root, |config| {
        let result = &mut config["control"]["results"][name];
        if result["requestId"].as_str() == Some(request_id) {
            result["state"] = serde_json::Value::String(state.into());
            result["result"] = serde_json::Value::String(message.into());
        }
        Ok(())
    });
}

#[cfg(windows)]
pub fn read_update_queue(root: &std::path::Path) -> serde_json::Value {
    read_update_queue_value(root)
}

#[cfg(not(windows))]
pub fn read_update_queue(_: &std::path::Path) -> serde_json::Value {
    serde_json::json!({"schemaVersion":1,"items":[],"run":null})
}

#[cfg(windows)]
pub fn check_update_cancelled(root: &std::path::Path) -> Result<(), String> {
    scheduled::check_update_cancelled(root)
}

#[cfg(windows)]
pub fn set_active_queue_item_state(
    root: &std::path::Path,
    id: &str,
    state: &str,
    message: Option<&str>,
) -> Result<(), String> {
    set_update_item_state(root, id, state, message)?;
    scheduled::write_status(root, state, message.unwrap_or(""));
    Ok(())
}

#[cfg(not(windows))]
pub fn set_active_queue_item_state(
    _: &std::path::Path,
    _: &str,
    _: &str,
    _: Option<&str>,
) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn check_update_cancelled(_: &std::path::Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn write_update_queue_value(
    root: &std::path::Path,
    value: &serde_json::Value,
) -> Result<(), String> {
    shroudforge_package::config::write_json(&update_queue_path(root), value)
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn update_update_queue<T>(
    root: &std::path::Path,
    update: impl FnOnce(&mut serde_json::Value) -> Result<T, String>,
) -> Result<T, String> {
    let _lock = shroudforge_package::config::installation_lock(root)?;
    let mut queue = read_update_queue_value(root);
    let result = update(&mut queue)?;
    write_update_queue_value(root, &queue)?;
    Ok(result)
}

#[cfg(windows)]
fn enqueue_update_item(root: &std::path::Path, item: serde_json::Value) -> Result<(), String> {
    let id = item["id"]
        .as_str()
        .ok_or("update queue item has no ID")?
        .to_owned();
    update_update_queue(root, |queue| {
        let items = queue["items"]
            .as_array_mut()
            .ok_or("update queue has an invalid items list")?;
        if let Some(existing) = items
            .iter_mut()
            .find(|existing| existing["id"].as_str() == Some(&id))
        {
            if existing["state"] == "complete" {
                *existing = item;
            }
        } else {
            items.push(item);
        }
        Ok(())
    })
}

#[cfg(windows)]
pub fn enqueue_system_update(
    root: &std::path::Path,
    release: serde_json::Value,
) -> Result<(), String> {
    let version = release["version"]
        .as_str()
        .ok_or("release has no version")?;
    if release["downloadUrl"]
        .as_str()
        .map_or(true, |url| !url.starts_with("https://"))
        || release["checksum"].as_str().is_none()
    {
        return Err("release download URL or checksum is missing".into());
    }
    enqueue_update_item(
        root,
        serde_json::json!({"id":format!("system:{version}"),"kind":"system","title":format!("ShroudForge {version}"),"version":version,"prerelease":release["prerelease"].as_bool().unwrap_or(false),"state":"queued","selected":true,"message":release["message"],"releaseUrl":release["releaseUrl"],"payload":release}),
    )
}

#[cfg(not(windows))]
pub fn enqueue_system_update(_: &std::path::Path, _: serde_json::Value) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(windows)]
pub fn select_update_queue_item(
    root: &std::path::Path,
    id: &str,
    selected: bool,
) -> Result<(), String> {
    update_update_queue(root, |queue| {
        let item = queue["items"]
            .as_array_mut()
            .and_then(|items| {
                items
                    .iter_mut()
                    .find(|item| item["id"].as_str() == Some(id))
            })
            .ok_or("update queue item was not found")?;
        if matches!(item["state"].as_str(), Some("downloading" | "installing")) {
            return Err("an active update cannot change selection".into());
        }
        item["selected"] = serde_json::Value::Bool(selected);
        Ok(())
    })
}

#[cfg(windows)]
pub fn select_update_queue_items(
    root: &std::path::Path,
    ids: &[String],
    selected: bool,
) -> Result<(), String> {
    update_update_queue(root, |queue| {
        let items = queue["items"]
            .as_array_mut()
            .ok_or("update queue has an invalid items list")?;
        for item in items {
            if ids
                .iter()
                .any(|id| item["id"].as_str() == Some(id.as_str()))
            {
                if matches!(item["state"].as_str(), Some("downloading" | "installing")) {
                    return Err("an active update cannot change selection".into());
                }
                item["selected"] = serde_json::Value::Bool(selected);
            }
        }
        Ok(())
    })
}

#[cfg(not(windows))]
pub fn select_update_queue_items(_: &std::path::Path, _: &[String], _: bool) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(windows)]
pub fn remove_update_queue_item(root: &std::path::Path, id: &str) -> Result<(), String> {
    update_update_queue(root, |queue| {
        let items = queue["items"]
            .as_array_mut()
            .ok_or("update queue has an invalid items list")?;
        if items.iter().any(|item| {
            item["id"].as_str() == Some(id)
                && matches!(item["state"].as_str(), Some("downloading" | "installing"))
        }) {
            return Err("an active update cannot be removed".into());
        }
        items.retain(|item| item["id"].as_str() != Some(id));
        Ok(())
    })
}

#[cfg(not(windows))]
pub fn remove_update_queue_item(_: &std::path::Path, _: &str) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn select_update_queue_item(_: &std::path::Path, _: &str, _: bool) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(windows)]
pub fn start_update_queue(
    root: &std::path::Path,
    ids: &[String],
    wait_for_game: bool,
    show_window: bool,
) -> Result<(), String> {
    let selected_ids = update_update_queue(root, |queue| {
        if queue["run"]["state"] == "running" {
            return Err("the update queue is already running".into());
        }
        let items = queue["items"]
            .as_array_mut()
            .ok_or("update queue has an invalid items list")?;
        let selected_ids = if ids.is_empty() {
            items
                .iter()
                .filter(|item| item["selected"].as_bool() == Some(true))
                .filter_map(|item| item["id"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        } else {
            ids.to_vec()
        };
        if selected_ids.is_empty() {
            return Err("select at least one update before starting the queue".into());
        }
        for id in &selected_ids {
            let item = items
                .iter_mut()
                .find(|item| item["id"].as_str() == Some(id.as_str()))
                .ok_or_else(|| format!("update queue item '{id}' was not found"))?;
            if matches!(item["state"].as_str(), Some("downloading" | "installing")) {
                return Err(format!("update queue item '{id}' is already active"));
            }
            item["state"] = serde_json::Value::String("queued".into());
            item["selected"] = serde_json::Value::Bool(true);
            item["waitForGame"] = serde_json::Value::Bool(wait_for_game);
        }
        queue["run"] = serde_json::json!({"state":"running","waitForGame":wait_for_game,"showWindow":show_window,"itemIds":selected_ids,"startedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
        Ok(selected_ids)
    })?;
    let _ = std::fs::remove_file(
        shroudforge_package::paths::updates_dir(root).join("cancel-system-update"),
    );
    scheduled::write_status(root, "queued", "Selected updates are queued");
    if let Err(error) = scheduled::start_system_waiter(root) {
        let _ = update_update_queue(root, |queue| {
            queue["run"] = serde_json::Value::Null;
            if let Some(items) = queue["items"].as_array_mut() {
                for item in items {
                    if selected_ids
                        .iter()
                        .any(|id| item["id"].as_str() == Some(id.as_str()))
                    {
                        item["state"] = serde_json::Value::String("error".into());
                        item["message"] = serde_json::Value::String(error.clone());
                    }
                }
            }
            Ok(())
        });
        return Err(error);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn start_update_queue(
    _: &std::path::Path,
    _: &[String],
    _: bool,
    _: bool,
) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(windows)]
pub fn clear_update_queue(root: &std::path::Path) -> Result<(), String> {
    let status =
        std::fs::read(shroudforge_package::paths::updates_dir(root).join("updater-status.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value["status"].as_str().map(str::to_owned));
    if status.as_deref() == Some("installing") {
        return Err("the update is being installed and can no longer be cancelled".into());
    }
    scheduled::clear_system_update(root)?;
    update_update_queue(root, |queue| {
        queue["items"] = serde_json::json!([]);
        queue["run"] = serde_json::Value::Null;
        Ok(())
    })
}

#[cfg(windows)]
pub fn cancel_update_queue(root: &std::path::Path) -> Result<(), String> {
    let status =
        std::fs::read(shroudforge_package::paths::updates_dir(root).join("updater-status.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value["status"].as_str().map(str::to_owned));
    if status.as_deref() == Some("installing") {
        return Err("the update is being installed and can no longer be cancelled".into());
    }
    update_update_queue(root, |queue| {
        if queue["run"]["state"] != "running" {
            return Err("there is no running update queue to cancel".into());
        }
        Ok(())
    })?;
    scheduled::clear_system_update(root)
}

#[cfg(not(windows))]
pub fn cancel_update_queue(_: &std::path::Path) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn clear_update_queue(_: &std::path::Path) -> Result<(), String> {
    Err("update queue is available on Windows only".into())
}

#[cfg(windows)]
pub fn request_system_stage(
    root: &std::path::Path,
    release: serde_json::Value,
    _wait_pid: u32,
    wait_for_game: bool,
) -> Result<(), String> {
    let version = release["version"]
        .as_str()
        .ok_or("release has no version")?
        .to_owned();
    enqueue_system_update(root, release)?;
    start_update_queue(root, &[format!("system:{version}")], wait_for_game, true)
}

#[cfg(windows)]
pub fn cancel_system_update(root: &std::path::Path) -> Result<(), String> {
    scheduled::clear_system_update(root)
}

#[cfg(not(windows))]
pub fn cancel_system_update(_: &std::path::Path) -> Result<(), String> {
    Err("system update cancellation is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn request_system_stage(
    _: &std::path::Path,
    _: serde_json::Value,
    _: u32,
    _: bool,
) -> Result<(), String> {
    Err("scheduled updater is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn request_mod_install(
    _: &std::path::Path,
    _: &str,
    _: &str,
    _: serde_json::Value,
) -> Result<(), String> {
    Err("scheduled updater is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn request_install_after_game(_: &std::path::Path, _: u32) -> Result<(), String> {
    Err("scheduled updater is available on Windows only".into())
}

#[cfg(windows)]
fn set_update_item_state(
    root: &std::path::Path,
    id: &str,
    state: &str,
    message: Option<&str>,
) -> Result<(), String> {
    update_update_queue(root, |queue| {
        let item = queue["items"]
            .as_array_mut()
            .and_then(|items| {
                items
                    .iter_mut()
                    .find(|item| item["id"].as_str() == Some(id))
            })
            .ok_or_else(|| format!("update queue item '{id}' disappeared"))?;
        item["state"] = serde_json::Value::String(state.into());
        item["message"] = message
            .map(|message| serde_json::Value::String(message.into()))
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    })
}

#[cfg(windows)]
fn run_update_queue(root: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let mut queue = read_update_queue_value(root);
    let run = queue["run"].clone();
    let ids = run["itemIds"]
        .as_array()
        .ok_or("update queue run has no item list")?
        .iter()
        .filter_map(|id| id.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let wait_for_game = run["waitForGame"].as_bool().unwrap_or(false);
    let show_window = run["showWindow"].as_bool().unwrap_or(true);
    if wait_for_game {
        scheduled::write_status(
            root,
            "waitingForGame",
            "Waiting for Enshrouded to close before starting selected downloads",
        );
        if let Err(error) = scheduled::wait_for_game_processes(root) {
            if error == "UPDATE_CANCELLED" {
                finish_cancelled_queue(root, &ids);
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
        if scheduled::check_update_cancelled(root).is_err() {
            finish_cancelled_queue(root, &ids);
            return Ok(());
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
        if item["state"] == "cancelled" {
            continue;
        }
        set_update_item_state(root, id, "downloading", None)?;
        let result = match item["kind"].as_str() {
            Some("system") => {
                let request = item["payload"].clone();
                let version = request["version"].as_str().unwrap_or_default();
                scheduled::write_status_with_version(
                    root,
                    "downloading",
                    "Downloading selected ShroudForge update",
                    Some(version),
                );
                scheduled::stage_system_update(root, &request).and_then(|()| {
                    if scheduled::check_update_cancelled(root).is_err() {
                        return Err("UPDATE_CANCELLED".into());
                    }
                    set_update_item_state(
                        root,
                        id,
                        "waitingForGame",
                        Some("Download verified; installation waits for Enshrouded to close"),
                    )?;
                    scheduled::write_status(
                        root,
                        "waitingForGame",
                        "Download verified; waiting to install ShroudForge",
                    );
                    let staged = shroudforge_package::paths::updates_dir(root).join("pending");
                    let status = std::process::Command::new(
                        std::env::current_exe().map_err(|error| error.to_string())?,
                    )
                    .args(["--update-worker", "--root"])
                    .arg(root)
                    .arg("--staged")
                    .arg(staged)
                    .status()
                    .map_err(|error| format!("could not launch validated installer: {error}"))?;
                    let _ = std::fs::remove_file(
                        shroudforge_package::paths::updates_dir(root).join("worker-queue.json"),
                    );
                    if status.success() {
                        Ok(())
                    } else if scheduled::check_update_cancelled(root).is_err() {
                        Err("UPDATE_CANCELLED".into())
                    } else {
                        Err(format!("installer returned {status}"))
                    }
                })
            }
            Some("mod") => {
                let payload = &item["payload"];
                let request = serde_json::json!({
                    "schemaVersion":1,
                    "queueItemId":id,
                    "operation":item["operation"],
                    "projectId":payload["projectId"],
                    "modId":payload["modId"],
                    "provider":payload["provider"]
                });
                let request_path =
                    shroudforge_package::paths::updates_dir(root).join("mod-install-queue.json");
                shroudforge_package::config::write_json(&request_path, &request)
                    .map_err(|error| error.to_string())?;
                let status =
                    std::process::Command::new(shroudforge_package::paths::loader_executable(root))
                        .args(["--catalog-install-worker", "--root"])
                        .arg(root)
                        .creation_flags(0x08000000)
                        .status()
                        .map_err(|error| format!("could not launch catalog installer: {error}"))?;
                if status.success() {
                    Ok(())
                } else if scheduled::check_update_cancelled(root).is_err() {
                    Err("UPDATE_CANCELLED".into())
                } else {
                    Err(format!("catalog installer returned {status}"))
                }
            }
            _ => Err(format!("unsupported update queue item kind for '{id}'")),
        };
        match result {
            Ok(()) => set_update_item_state(root, id, "complete", Some("Update completed"))?,
            Err(error) if error == "UPDATE_CANCELLED" => {
                finish_cancelled_queue(root, &ids);
                return Ok(());
            }
            Err(error) => {
                set_update_item_state(root, id, "error", Some(&error))?;
                failures.push(format!("{id}: {error}"));
            }
        }
    }
    update_update_queue(root, |queue| {
        queue["run"]["state"] = serde_json::Value::String("complete".into());
        queue["run"]["finishedAt"] = serde_json::json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        Ok(())
    })?;
    let _ = std::fs::remove_file(
        shroudforge_package::paths::updates_dir(root).join("cancel-system-update"),
    );
    let contains_system_update = ids.iter().any(|id| id.starts_with("system:"));
    if failures.is_empty() {
        if contains_system_update {
            scheduled::write_status(root, "installed", "Selected update queue completed");
        } else {
            scheduled::write_status(root, "ready", "Selected mod queue completed");
        }
        Ok(())
    } else {
        if contains_system_update {
            scheduled::write_status(root, "error", &failures.join("; "));
        } else {
            scheduled::write_status(root, "ready", "One or more mod queue items failed");
        }
        Err(failures.join("; "))
    }
}

#[cfg(windows)]
fn finish_cancelled_queue(root: &std::path::Path, ids: &[String]) {
    for id in ids {
        let queue = read_update_queue_value(root);
        let state = queue["items"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["id"].as_str() == Some(id)))
            .and_then(|item| item["state"].as_str());
        if !matches!(state, Some("complete" | "error")) {
            let _ = set_update_item_state(root, id, "cancelled", Some("Cancelled by user"));
        }
    }
    let _ = update_update_queue(root, |queue| {
        queue["run"]["state"] = serde_json::Value::String("cancelled".into());
        queue["run"]["finishedAt"] = serde_json::json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        Ok(())
    });
    let updates = shroudforge_package::paths::updates_dir(root);
    let _ = std::fs::remove_file(updates.join("mod-install-queue.json"));
    let _ = std::fs::remove_file(updates.join("worker-queue.json"));
    let _ = std::fs::remove_file(updates.join("system-stage-request.json"));
    let _ = std::fs::remove_file(updates.join("download.zip"));
    let _ = std::fs::remove_dir_all(updates.join("pending-download"));
    let _ = std::fs::remove_file(updates.join("pending.ready"));
    scheduled::write_status(root, "cancelled", "Selected update queue cancelled");
    let _ = std::fs::remove_file(updates.join("cancel-system-update"));
}

#[cfg(windows)]
pub fn run_scheduled_worker() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| args.windows(2).find(|p| p[0] == name).map(|p| p[1].clone());
    let root = std::path::PathBuf::from(value("--root").ok_or("missing --root")?);
    let update_queue = read_update_queue_value(&root);
    if update_queue["run"]["state"] == "running" {
        return run_update_queue(&root);
    }
    let queue_path = shroudforge_package::paths::updates_dir(&root).join("worker-queue.json");
    let mut first_error = None;
    let stage_request =
        shroudforge_package::paths::updates_dir(&root).join("system-stage-request.json");
    if stage_request.is_file() {
        let result = (|| {
            let bytes = std::fs::read(&stage_request)
                .map_err(|e| format!("cannot read system stage request: {e}"))?;
            let request: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("invalid system stage request: {e}"))?;
            if request["waitForGame"]
                .as_bool()
                .unwrap_or_else(|| request["waitPid"].as_u64().unwrap_or_default() != 0)
            {
                scheduled::write_status(
                    &root,
                    "waitingForGame",
                    "Waiting for Enshrouded to close before preparing the update",
                );
                scheduled::wait_for_game_processes(&root)?;
            }
            scheduled::open_update_window(&root)?;
            scheduled::write_status(
                &root,
                "downloading",
                "Downloading and verifying the system update",
            );
            scheduled::stage_system_update(&root, &request)
        })();
        match &result {
            Ok(()) => scheduled::write_status(
                &root,
                "staged",
                "Verified and staged; installation waits for game exit",
            ),
            Err(error) if error == "UPDATE_CANCELLED" => {
                let updates = shroudforge_package::paths::updates_dir(&root);
                let _ = std::fs::remove_file(updates.join("download.zip"));
                let _ = std::fs::remove_dir_all(updates.join("pending-download"));
                let _ = std::fs::remove_dir_all(updates.join("pending"));
                let _ = std::fs::remove_file(updates.join("pending.ready"));
                scheduled::write_status(
                    &root,
                    "cancelled",
                    "Update download cancelled; queue cleared",
                );
            }
            Err(error) => {
                scheduled::write_status(&root, "error", error);
                let _ = shroudforge_package::logging::append(
                    &root,
                    'E',
                    "updater",
                    &format!("System update download failed: {error}"),
                );
                first_error = Some(error.clone());
            }
        }
        let _ = std::fs::remove_file(&stage_request);
        let _ = std::fs::remove_file(
            shroudforge_package::paths::updates_dir(&root).join("cancel-system-update"),
        );
    }
    // Catalog mod packages are independent of the ShroudForge binary release.
    // Complete these while the game may still be running, before a full-package
    // installer can wait for the game to exit and replace the loader executable.
    let mod_queue = shroudforge_package::paths::updates_dir(&root).join("mod-install-queue.json");
    if mod_queue.is_file() {
        let result =
            std::process::Command::new(shroudforge_package::paths::loader_executable(&root))
                .args(["--catalog-install-worker", "--root"])
                .arg(&root)
                .creation_flags(0x08000000)
                .status()
                .map_err(|e| format!("could not launch catalog install worker: {e}"))
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(format!("catalog install worker returned {status}"))
                    }
                });
        if let Err(ref error) = result {
            let _ = shroudforge_package::logging::append(
                &root,
                'E',
                "updater",
                &format!("Catalog install failed: {error}"),
            );
            if first_error.is_none() {
                first_error = Some(error.clone());
            }
        }
    }
    if queue_path.is_file() {
        let result = (|| {
            let bytes = std::fs::read(&queue_path)
                .map_err(|e| format!("cannot read updater queue: {e}"))?;
            let queue: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("invalid updater queue: {e}"))?;
            if queue["operation"] != "installPending" {
                return Err("unsupported updater operation".into());
            }
            let staged = shroudforge_package::paths::updates_dir(&root).join("pending");
            let status =
                std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                    .args(["--update-worker", "--root"])
                    .arg(&root)
                    .arg("--staged")
                    .arg(&staged)
                    .status()
                    .map_err(|e| format!("could not launch validated installer: {e}"))?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("installer returned {status}"))
            }
        })();
        if let Err(ref error) = result {
            scheduled::write_status(&root, "error", error);
            let _ = shroudforge_package::logging::append(
                &root,
                'E',
                "updater",
                &format!("Scheduled system updater failed: {error}"),
            );
            first_error = Some(error.clone());
        }
        let _ = std::fs::remove_file(&queue_path);
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(windows)]
pub fn run_module() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
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
        return scheduled::open_update_window(&root).map_err(|error| {
            let _ = shroudforge_package::logging::append(&root, 'E', "updater-window", &error);
            error
        });
    }
    if args.len() == 1 {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let module_directory = executable
            .parent()
            .ok_or("updater executable has no module directory")?;
        let root = module_directory
            .parent()
            .ok_or("updater executable is not inside the ShroudForge module directory")?
            .to_path_buf();
        return scheduled::open_update_window(&root).map_err(|error| {
            let _ = shroudforge_package::logging::append(&root, 'E', "updater-window", &error);
            error
        });
    }
    windows::run()
}

#[cfg(windows)]
pub fn request_gamefiles_restore(root: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let status_path =
        shroudforge_package::paths::backups_dir(root).join("gamefiles-restore-status.json");
    let status = shroudforge_package::backups::originals_status(
        root,
        if root.join("enshrouded_server.exe").is_file() {
            "server"
        } else {
            "client"
        },
    );
    if status["status"] != "ready" {
        return Err("verified original game files are not available".into());
    }
    if let Ok(bytes) = std::fs::read(&status_path) {
        if let Ok(previous) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if matches!(
                previous["status"].as_str(),
                Some("queued" | "waitingForGame" | "restoring")
            ) {
                return Err("gamefile restoration is already in progress".into());
            }
        }
    }
    shroudforge_package::config::write_json(
        &status_path,
        &serde_json::json!({"schemaVersion":1,"status":"queued","message":"Gamefile restoration queued"}),
    )?;
    let _ = std::fs::remove_file(
        shroudforge_package::paths::updates_dir(root).join("cancel-system-update"),
    );
    let result = std::process::Command::new(shroudforge_package::paths::updater_executable(root))
        .args(["--restore-gamefiles-worker", "--root"])
        .arg(root)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| format!("could not start gamefile restoration in the updater: {error}"));
    if result.is_err() {
        let _ = std::fs::remove_file(status_path);
    }
    result.map(|_| ())
}

#[cfg(not(windows))]
pub fn request_gamefiles_restore(_: &std::path::Path) -> Result<(), String> {
    Err("gamefile restoration is available on Windows only".into())
}

#[cfg(not(windows))]
pub fn run_module() -> Result<(), String> {
    Err("ShroudForge updater is available on Windows only".into())
}
