#[cfg(windows)]
mod windows {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };

    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
    pub fn run() -> Result<(), String> {
        let arguments = Arguments::read()?;
        wait_for_process(arguments.wait_pid)?;
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
        let process = unsafe { OpenProcess(SYNCHRONIZE_ACCESS, 0, pid) };
        if process.is_null() {
            return Ok(());
        }
        let result = unsafe { WaitForSingleObject(process, 120_000) };
        unsafe { CloseHandle(process) };
        if result != WAIT_OBJECT_0 {
            return Err("timed out waiting for Enshrouded to exit".into());
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
pub fn run_module() -> Result<(), String> {
    windows::run()
}

#[cfg(not(windows))]
pub fn run_module() -> Result<(), String> {
    Err("ShroudForge updater is available on Windows only".into())
}
