#[cfg(windows)]
mod windows {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };

    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
    pub fn run() -> Result<(), String> {
        let arguments = Arguments::read()?;
        super::scheduled::write_status(&arguments.root, "waitingForGame", "Update is verified; waiting for Enshrouded to exit");
        append_log(
            &arguments.root,
            'I',
            &format!("Independent updater is waiting for game process {} to exit", arguments.wait_pid),
        );
        if let Err(error) = wait_for_process(arguments.wait_pid) {
            append_log(&arguments.root, 'E', &format!("Game exit could not be confirmed; update was not installed: {error}"));
            return Err(error);
        }
        if let Err(error) = super::scheduled::wait_for_game_processes(&arguments.root) {
            append_log(&arguments.root, 'E', &format!("Game process scan failed; update was not installed: {error}"));
            return Err(error);
        }
        super::scheduled::write_status(&arguments.root, "installing", "Installing the verified ShroudForge update");
        validate_roots(&arguments.root, &arguments.staged)?;
        let source = arguments.staged.clone();
        if !source.join("shroudforge/version.json").is_file() {
            return Err("staged release does not contain shroudforge/version.json".into());
        }

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let backup = arguments
            .root
            .join("shroudforge/updates/backups")
            .join(stamp.to_string());
        fs::create_dir_all(&backup).map_err(|error| error.to_string())?;
        append_log(
            &arguments.root,
            'I',
            &format!("Starting update from {}", arguments.staged.display()),
        );

        let paths = managed_paths(&source)?;
        // Complete and verify backup before the first installed file changes.
        for relative in &paths {
            validate_file_path(&source, relative)?;
            validate_file_path(&arguments.root, relative)?;
            let incoming = source.join(relative);
            if !incoming.is_file() {
                return Err(format!("missing release file: {relative}"));
            }
            let current = arguments.root.join(relative);
            if current.exists() {
                copy_entry(&current, &backup.join(relative))?;
            }
        }
        let result = apply(&source, &arguments.root, &paths);
        match result {
            Ok(()) => {
                let release: serde_json::Value = serde_json::from_slice(
                    &fs::read(source.join("shroudforge/version.json")).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let state = serde_json::json!({
                    "schemaVersion": 1, "status": "installed", "version": release["version"],
                    "build": release["build"], "installedAt": stamp, "backup": backup
                });
                if let Err(error) = shroudforge_package::config::write_document(&arguments.root, "state", &state) {
                    append_log(
                        &arguments.root,
                        'E',
                        &format!("Installed files, but could not save update state: {error}"),
                    );
                }
                let _ = fs::remove_file(arguments.root.join("shroudforge/updates/pending.ready"));
                let status = serde_json::json!({"schemaVersion":1,"operation":"systemStage","status":"installed","step":"complete","message":"System update installed successfully","version":release["version"],"updatedAt":std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()});
                let _ = shroudforge_package::config::write_json(&arguments.root.join("shroudforge/updates/updater-status.json"), &status);
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
                if let Err(rollback_error) = restore(&backup, &arguments.root, &paths) {
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
        wait_pid: u32,
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
                wait_pid: value("--wait-pid")
                    .ok_or("missing --wait-pid")?
                    .parse()
                    .map_err(|_| "invalid --wait-pid")?,
            })
        }
    }

    fn wait_for_process(pid: u32) -> Result<(), String> {
        if pid == 0 { return Ok(()); }
        let process = unsafe { OpenProcess(SYNCHRONIZE_ACCESS, 0, pid) };
        if process.is_null() {
            let error = unsafe { GetLastError() };
            return if error == ERROR_INVALID_PARAMETER {
                Ok(())
            } else {
                Err(format!("cannot confirm game process {pid} has exited (Windows error {error})"))
            };
        }
        let result = loop {
            let result = unsafe { WaitForSingleObject(process, 1_000) };
            if result == WAIT_TIMEOUT { continue; }
            break result;
        };
        unsafe { CloseHandle(process) };
        if result != WAIT_OBJECT_0 {
            return Err("could not confirm Enshrouded process exit".into());
        }
        Ok(())
    }

    fn validate_roots(root: &Path, staged: &Path) -> Result<(), String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("invalid install root: {error}"))?;
        let staged = staged
            .canonicalize()
            .map_err(|error| format!("invalid staged root: {error}"))?;
        let updates = root.join("shroudforge/updates");
        if !staged.starts_with(&updates) {
            return Err("staged update is outside shroudforge/updates".into());
        }
        if root == staged {
            return Err("staged update must not equal install root".into());
        }
        Ok(())
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
            let path = Path::new(relative);
            let allowed = matches!(
                relative,
                "winmm.dll"
                    | "kfc-runtime.dll"
                    | "shroudforge-runtime.dll"
                    | "shroudforge.exe"
                    | "shroudforge-updater.exe"
                    | "shroudforge/version.json"
            ) || relative.starts_with("mods/");
            if !allowed
                || relative.is_empty()
                || relative.contains('\\')
                || relative.contains(':')
                || !path
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
                || matches!(
                    relative,
                    "shroudforge/config/shroudforge.json"
                        | "shroudforge/config/state.json"
                        | "shroudforge/config/.shroudforge-write.lock"
                )
            {
                return Err(format!("unsafe managed path: {relative}"));
            }
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

    fn validate_file_path(root: &Path, relative: &str) -> Result<(), String> {
        use std::os::windows::fs::MetadataExt;
        let mut path = root.to_path_buf();
        // Never follow a junction/symlink into another installation or user directory.
        for component in Path::new(relative).components() {
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
            let current = target.join(relative);
            if relative.starts_with("mods/") && relative.ends_with("/mod.json") && current.is_file() {
                let previous = serde_json::from_slice(&fs::read(&current).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                let next = serde_json::from_slice(&fs::read(&incoming).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                let merged = shroudforge_package::config::merge_mod_update(target, previous, next)?;
                shroudforge_package::config::write_json(&current, &merged)?;
            } else {
                copy_entry(&incoming, &current)?;
            }
        }
        Ok(())
    }

    fn restore(backup: &Path, target: &Path, paths: &[String]) -> Result<(), String> {
        for relative in paths {
            let saved = backup.join(relative);
            let current = target.join(relative);
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
        time::UNIX_EPOCH,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER},
        System::{
            Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS},
            Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    };

    const MAX_UPDATE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const MAX_ARCHIVE_ENTRIES: usize = 4096;

    pub(super) fn write_status(root: &Path, status: &str, message: &str) {
        write_status_with_version(root, status, message, None);
    }

    pub(super) fn write_status_with_version(root: &Path, status: &str, message: &str, version: Option<&str>) {
        let previous_path = root.join("shroudforge/updates/updater-status.json");
        let previous = fs::read(&previous_path).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let keep_progress = status != "queued";
        let version = version.map(str::to_owned).or_else(|| previous.as_ref().and_then(|value| value["version"].as_str()).map(str::to_owned));
        let downloaded = if keep_progress { previous.as_ref().and_then(|value| value["downloadedBytes"].as_u64()).unwrap_or(0) } else { 0 };
        let total = if keep_progress { previous.as_ref().and_then(|value| value.get("totalBytes")).cloned().unwrap_or(serde_json::Value::Null) } else { serde_json::Value::Null };
        let speed = if keep_progress { previous.as_ref().and_then(|value| value.get("bytesPerSecond")).cloned().unwrap_or(serde_json::Value::Null) } else { serde_json::Value::Null };
        let value = serde_json::json!({"schemaVersion":1,"operation":"systemStage","status":status,"step":status,"message":message,"version":version,"downloadedBytes":downloaded,"totalBytes":total,"bytesPerSecond":speed,"updatedAt":std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()});
        let _ = shroudforge_package::config::write_json(&root.join("shroudforge/updates/updater-status.json"), &value);
    }

    fn write_download_progress(root: &Path, version: &str, downloaded: u64, total: Option<u64>, speed: Option<u64>) {
        let value = serde_json::json!({"schemaVersion":1,"operation":"systemStage","status":"downloading","step":"download","message":"Downloading ShroudForge update","version":version,"downloadedBytes":downloaded,"totalBytes":total,"bytesPerSecond":speed,"updatedAt":std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()});
        let _ = shroudforge_package::config::write_json(&root.join("shroudforge/updates/updater-status.json"), &value);
    }

    pub(super) fn stage_system_update(root: &Path, request: &serde_json::Value) -> Result<(), String> {
        let version = request["version"].as_str().ok_or("stage request has no version")?;
        let url = request["downloadUrl"].as_str().ok_or("stage request has no download URL")?;
        let checksum = request["checksum"].as_str().ok_or("stage request has no checksum")?.to_ascii_lowercase();
        let wait_pid = request["waitPid"].as_u64().unwrap_or(0) as u32;
        if !url.starts_with("https://") || checksum.len() != 64 || !checksum.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("stage request URL or SHA-256 checksum is invalid".into());
        }
        let updates = root.join("shroudforge/updates");
        fs::create_dir_all(&updates).map_err(|e| e.to_string())?;
        let download = updates.join("download.zip");
        let extraction = updates.join("pending-download");
        let pending = updates.join("pending");
        let _ = fs::remove_dir_all(&extraction);
        fs::create_dir_all(&extraction).map_err(|e| e.to_string())?;
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(120)).user_agent(format!("ShroudForge/{version} updater")).build().map_err(|e| e.to_string())?;
        let mut response = client.get(url).send().map_err(|e| format!("download failed: {e}"))?;
        if !response.status().is_success() { return Err(format!("release download returned HTTP {}", response.status())); }
        let content_length = response.content_length();
        if content_length.is_some_and(|length| length > MAX_UPDATE_BYTES) { return Err("update package exceeds 2 GiB".into()); }
        let mut file = fs::File::create(&download).map_err(|e| e.to_string())?;
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        let mut total = 0u64;
        let mut last_progress = std::time::Instant::now();
        let mut last_bytes = 0u64;
        write_download_progress(root, version, 0, content_length, Some(0));
        let mut buffer = [0u8; 1024 * 1024];
        loop {
            let count = response.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 { break; }
            total = total.saturating_add(count as u64);
            if total > MAX_UPDATE_BYTES { return Err("update package exceeds 2 GiB".into()); }
            file.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
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
        write_status(root, "verifying", "Verifying the downloaded package checksum");
        if format!("{:x}", hasher.finalize()) != checksum { return Err("update package SHA-256 checksum does not match".into()); }
        write_status(root, "extracting", "Extracting and validating update files");
        let source = fs::File::open(&download).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(source).map_err(|e| e.to_string())?;
        if archive.len() > MAX_ARCHIVE_ENTRIES { return Err("update archive contains too many entries".into()); }
        let mut expanded = 0u64;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
            let relative = entry.enclosed_name().ok_or_else(|| format!("unsafe archive path: {}", entry.name()))?.to_path_buf();
            if relative.components().any(|part| !matches!(part, std::path::Component::Normal(_))) { return Err(format!("unsafe archive path: {}", entry.name())); }
            expanded = expanded.saturating_add(entry.size());
            if expanded > 8 * MAX_UPDATE_BYTES { return Err("expanded update archive exceeds 16 GiB".into()); }
            let output = extraction.join(relative);
            if entry.is_dir() { fs::create_dir_all(&output).map_err(|e| e.to_string())?; }
            else {
                if let Some(parent) = output.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                let mut file = fs::File::create(&output).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
                file.flush().map_err(|e| e.to_string())?;
            }
        }
        for required in ["shroudforge/version.json", "shroudforge.exe", "shroudforge-updater.exe"] {
            if !extraction.join(required).is_file() { return Err(format!("update package is missing {required}")); }
        }
        let _ = fs::remove_file(updates.join("pending.ready"));
        if pending.exists() { fs::remove_dir_all(&pending).map_err(|e| e.to_string())?; }
        fs::rename(&extraction, &pending).map_err(|e| format!("could not promote verified update: {e}"))?;
        let ready = serde_json::json!({"version":version,"checksumAlgorithm":"SHA-256","checksum":checksum,"stagedAt":std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()});
        shroudforge_package::config::write_json(&updates.join("pending.ready"), &ready)?;
        let _ = fs::remove_file(download);
        let queue = serde_json::json!({"schemaVersion":1,"operation":"installPending","waitPid":wait_pid});
        shroudforge_package::config::write_json(&updates.join("worker-queue.json"), &queue)?;
        Ok(())
    }

    pub(super) fn wait_for_game_processes(root: &Path) -> Result<(), String> {
        use std::{mem::size_of, time::Duration};
        let root = root.canonicalize().map_err(|e| format!("invalid install root for process scan: {e}"))?;
        let prefix = format!("{}\\", root.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase());
        loop {
            let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
            if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE { return Err(format!("could not enumerate game processes (Windows error {})", unsafe { GetLastError() })); }
            let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut running = false;
            let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
            while has_entry {
                let exe = String::from_utf16_lossy(&entry.szExeFile[..entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len())]);
                if exe.eq_ignore_ascii_case("enshrouded.exe") || exe.eq_ignore_ascii_case("enshrouded_server.exe") {
                    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000, 0, entry.th32ProcessID) };
                    if process.is_null() {
                        let error = unsafe { GetLastError() };
                        if error != ERROR_INVALID_PARAMETER { unsafe { CloseHandle(snapshot) }; return Err(format!("cannot verify Enshrouded process {} (Windows error {error})", entry.th32ProcessID)); }
                    } else {
                        let mut path = vec![0u16; 32768];
                        let mut length = path.len() as u32;
                        let queried = unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) } != 0;
                        unsafe { CloseHandle(process) };
                        if !queried { unsafe { CloseHandle(snapshot) }; return Err(format!("cannot read image path for Enshrouded process {}", entry.th32ProcessID)); }
                        let path = String::from_utf16_lossy(&path[..length as usize]).to_lowercase();
                        if path.starts_with(&prefix) { running = true; break; }
                    }
                }
                has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
            }
            unsafe { CloseHandle(snapshot) };
            if !running { return Ok(()); }
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
        let source = root.join("shroudforge-updater.exe");
        if !source.is_file() { return Err(format!("updater executable missing: {}", source.display())); }
        let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
        let modified = fs::metadata(&source).and_then(|m| m.modified()).map_err(|e| e.to_string())?.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let destination = PathBuf::from(local).join("ShroudForge").join("Updater").join(task_id(root)?).join(modified.to_string()).join("shroudforge-updater.exe");
        if let Some(parent) = destination.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        if !destination.is_file() { fs::copy(&source, &destination).map_err(|e| format!("cannot install independent updater: {e}"))?; }
        Ok(destination)
    }

    fn update_window_task_id(root: &Path) -> Result<String, String> {
        Ok(format!("{}_Window", task_id(root)?))
    }

    fn ensure_update_window_task(root: &Path) -> Result<String, String> {
        let executable = task_executable(root)?;
        let name = update_window_task_id(root)?;
        let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
        let action = format!("\"{}\" --open-window --root \"{}\"", executable.display(), canonical_root.display());
        let output = Command::new("schtasks.exe")
            .args(["/Create", "/SC", "ONCE", "/ST", "23:59", "/TN", &name, "/TR", &action, "/F", "/RL", "LIMITED", "/IT"])
            .output().map_err(|error| format!("could not register update window: {error}"))?;
        if !output.status.success() {
            return Err(format!("could not register update window: {}", String::from_utf8_lossy(&output.stderr).trim()));
        }
        Ok(name)
    }

    pub fn request_update_window(root: &Path) -> Result<(), String> {
        let name = ensure_update_window_task(root)?;
        let output = Command::new("schtasks.exe").args(["/Run", "/TN", &name]).output()
            .map_err(|error| format!("could not open update window: {error}"))?;
        if !output.status.success() {
            let _ = Command::new("schtasks.exe").args(["/Delete", "/TN", &name, "/F"]).output();
            return Err(format!("could not open update window: {}", String::from_utf8_lossy(&output.stderr).trim()));
        }
        Ok(())
    }

    pub fn open_update_window(root: &Path) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let source = root.join("shroudforge.exe");
        if !source.is_file() {
            return Err(format!("ShroudForge UI executable is missing: {}", source.display()));
        }
        let updater = task_executable(root)?;
        let directory = updater.parent().ok_or("updater cache has no parent directory")?;
        let metadata = fs::metadata(&source).map_err(|error| error.to_string())?;
        let changed = metadata.modified().map_err(|error| error.to_string())?.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let ui = directory.join(format!("shroudforge-ui-{changed}.exe"));
        if !ui.is_file() { fs::copy(&source, &ui).map_err(|error| format!("could not prepare independent update window: {error}"))?; }
        let target = if root.join("enshrouded.exe").is_file() { "client" } else { "server" };
        Command::new(&ui)
            .args(["--module-ui", "--desktop", "--updater-window", "--root"])
            .arg(root)
            .args(["--target", target])
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|error| format!("could not launch independent update window: {error}"))?;
        if let Ok(name) = update_window_task_id(root) {
            let _ = Command::new("schtasks.exe").args(["/Delete", "/TN", &name, "/F"]).output();
        }
        Ok(())
    }

    fn ensure_task(root: &Path) -> Result<String, String> {
        let executable = task_executable(root)?;
        let name = task_id(root)?;
        let action = format!("\"{}\" --run-queue --root \"{}\"", executable.display(), root.canonicalize().map_err(|e| e.to_string())?.display());
        let output = Command::new("schtasks.exe").args(["/Create", "/SC", "ONLOGON", "/TN", &name, "/TR", &action, "/F", "/RL", "LIMITED", "/IT"]).output().map_err(|e| format!("could not register updater task: {e}"))?;
        if !output.status.success() { return Err(format!("could not register updater task: {}", String::from_utf8_lossy(&output.stderr).trim())); }
        Ok(name)
    }

    pub fn request(root: &Path) -> Result<(), String> {
        let name = ensure_task(root)?;
        let output = Command::new("schtasks.exe").args(["/Run", "/TN", &name]).output().map_err(|e| format!("could not start scheduled updater: {e}"))?;
        if !output.status.success() { return Err(format!("could not start scheduled updater: {}", String::from_utf8_lossy(&output.stderr).trim())); }
        Ok(())
    }
}

#[cfg(windows)]
pub fn request_worker(root: &std::path::Path) -> Result<(), String> { scheduled::request(root) }

#[cfg(not(windows))]
pub fn request_worker(_: &std::path::Path) -> Result<(), String> { Err("scheduled updater is available on Windows only".into()) }

#[cfg(windows)]
pub fn request_update_window(root: &std::path::Path) -> Result<(), String> { scheduled::request_update_window(root) }

#[cfg(not(windows))]
pub fn request_update_window(_: &std::path::Path) -> Result<(), String> { Err("scheduled updater is available on Windows only".into()) }

#[cfg(windows)]
pub fn request_install_after_game(root: &std::path::Path, pid: u32) -> Result<(), String> {
    let request = serde_json::json!({"schemaVersion":1,"operation":"installPending","waitPid":pid});
    shroudforge_package::config::write_json(&root.join("shroudforge/updates/worker-queue.json"), &request).map_err(|e| e.to_string())?;
    request_worker(root)
}

#[cfg(windows)]
pub fn request_mod_install(root: &std::path::Path, project_id: &str, provider: serde_json::Value) -> Result<(), String> {
    if project_id.is_empty() || project_id.len() > 128 || !project_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) {
        return Err("invalid mod project ID".into());
    }
    let path = root.join("shroudforge/updates/mod-install-queue.json");
    if path.exists() {
        let existing = std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        if existing.as_ref().and_then(|value| value["projectId"].as_str()) != Some(project_id) {
            return Err("another catalog install is already queued or running".into());
        }
        return request_worker(root);
    }
    let request = serde_json::json!({"schemaVersion":1,"projectId":project_id,"provider":provider});
    shroudforge_package::config::write_json(&path, &request).map_err(|e| e.to_string())?;
    request_worker(root).map_err(|error| format!("install request was saved but the scheduled updater could not be started: {error}"))?;
    Ok(())
}

#[cfg(windows)]
pub fn request_system_stage(root: &std::path::Path, release: serde_json::Value, wait_pid: u32) -> Result<(), String> {
    let path = root.join("shroudforge/updates/system-stage-request.json");
    if path.exists() {
        let existing = std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        if existing.as_ref().and_then(|value| value["version"].as_str()) != release["version"].as_str() {
            return Err("another system update download is already queued or running".into());
        }
        return request_worker(root);
    }
    let request = serde_json::json!({"schemaVersion":1,"version":release["version"],"downloadUrl":release["downloadUrl"],"checksum":release["checksum"],"waitPid":wait_pid});
    shroudforge_package::config::write_json(&path, &request).map_err(|e| e.to_string())?;
    scheduled::write_status_with_version(root, "queued", "System update download queued in independent updater", release["version"].as_str());
    request_worker(root).map_err(|error| format!("download request was saved but the scheduled updater could not be started: {error}"))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn request_system_stage(_: &std::path::Path, _: serde_json::Value, _: u32) -> Result<(), String> { Err("scheduled updater is available on Windows only".into()) }

#[cfg(not(windows))]
pub fn request_mod_install(_: &std::path::Path, _: &str, _: serde_json::Value) -> Result<(), String> { Err("scheduled updater is available on Windows only".into()) }

#[cfg(not(windows))]
pub fn request_install_after_game(_: &std::path::Path, _: u32) -> Result<(), String> { Err("scheduled updater is available on Windows only".into()) }

#[cfg(windows)]
pub fn run_scheduled_worker() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| args.windows(2).find(|p| p[0] == name).map(|p| p[1].clone());
    let root = std::path::PathBuf::from(value("--root").ok_or("missing --root")?);
    let queue_path = root.join("shroudforge/updates/worker-queue.json");
    let mut first_error = None;
    let stage_request = root.join("shroudforge/updates/system-stage-request.json");
    if stage_request.is_file() {
        let result = (|| {
            let bytes = std::fs::read(&stage_request).map_err(|e| format!("cannot read system stage request: {e}"))?;
            let request: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("invalid system stage request: {e}"))?;
            scheduled::write_status(&root, "downloading", "Downloading and verifying the system update");
            scheduled::stage_system_update(&root, &request)
        })();
        match &result {
            Ok(()) => scheduled::write_status(&root, "staged", "Verified and staged; installation waits for game exit"),
            Err(error) => {
                scheduled::write_status(&root, "error", error);
                let _ = shroudforge_package::logging::append(&root, 'E', "updater", &format!("System update download failed: {error}"));
                first_error = Some(error.clone());
            }
        }
        let _ = std::fs::remove_file(&stage_request);
    }
    if queue_path.is_file() {
        let result = (|| {
            let bytes = std::fs::read(&queue_path).map_err(|e| format!("cannot read updater queue: {e}"))?;
            let queue: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("invalid updater queue: {e}"))?;
            if queue["operation"] != "installPending" { return Err("unsupported updater operation".into()); }
            let pid = queue["waitPid"].as_u64().ok_or("updater queue has no game PID")? as u32;
            let staged = root.join("shroudforge/updates/pending");
            let status = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .args(["--update-worker", "--root"])
                .arg(&root).arg("--staged").arg(&staged).arg("--wait-pid").arg(pid.to_string())
                .status().map_err(|e| format!("could not launch validated installer: {e}"))?;
            if status.success() { Ok(()) } else { Err(format!("installer returned {status}")) }
        })();
        if let Err(ref error) = result {
            scheduled::write_status(&root, "error", error);
            let _ = shroudforge_package::logging::append(&root, 'E', "updater", &format!("Scheduled system updater failed: {error}"));
            first_error = Some(error.clone());
        }
        let _ = std::fs::remove_file(&queue_path);
    }
    let mod_queue = root.join("shroudforge/updates/mod-install-queue.json");
    if mod_queue.is_file() {
        let result = std::process::Command::new(root.join("shroudforge.exe"))
            .args(["--catalog-install-worker", "--root"])
            .arg(&root)
            .creation_flags(0x08000000)
            .status().map_err(|e| format!("could not launch catalog install worker: {e}"))
            .and_then(|status| if status.success() { Ok(()) } else { Err(format!("catalog install worker returned {status}")) });
        if let Err(ref error) = result {
            let _ = shroudforge_package::logging::append(&root, 'E', "updater", &format!("Catalog install failed: {error}"));
            if first_error.is_none() { first_error = Some(error.clone()); }
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(windows)]
pub fn run_module() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--run-queue") { return run_scheduled_worker(); }
    if args.iter().any(|a| a == "--open-window") {
        let root = args.windows(2).find(|pair| pair[0] == "--root").map(|pair| std::path::PathBuf::from(&pair[1])).ok_or("missing --root")?;
        return scheduled::open_update_window(&root).map_err(|error| {
            let _ = shroudforge_package::logging::append(&root, 'E', "updater-window", &error);
            error
        });
    }
    if args.len() == 1 {
        let root = std::env::current_exe().map_err(|error| error.to_string())?.parent().ok_or("updater executable has no installation directory")?.to_path_buf();
        return scheduled::request_update_window(&root).map_err(|error| {
            let _ = shroudforge_package::logging::append(&root, 'E', "updater-window", &error);
            error
        });
    }
    windows::run()
}

#[cfg(not(windows))]
pub fn run_module() -> Result<(), String> {
    Err("ShroudForge updater is available on Windows only".into())
}
