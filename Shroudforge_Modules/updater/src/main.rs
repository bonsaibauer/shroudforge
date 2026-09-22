#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("ShroudForge Updater is available on Windows only");
}

#[cfg(windows)]
mod windows {
    use std::{
        fs,
        io::Write,
        path::{Path, PathBuf},
        sync::OnceLock,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };

    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
    static STARTED: OnceLock<Instant> = OnceLock::new();
    const MANAGED_PATHS: &[&str] = &[
        "winmm.dll",
        "shroudforge-runtime.dll",
        "shroudforge.exe",
        "version.json",
        "Shroudforge_Modules",
        "Shroudforge_Updater",
        "mods/mod.flight",
        "mods/mod.infinite-item-split",
        "mods/mod.infinite-item-use",
        "mods/mod.no-fall-damage",
        "mods/mod.no-resource-cost",
        "mods/mod.no-stamina-loss",
        "mods/mod.unlock-blueprints",
    ];

    pub fn run() -> Result<(), String> {
        STARTED.get_or_init(Instant::now);
        let arguments = Arguments::read()?;
        wait_for_process(arguments.wait_pid)?;
        validate_roots(&arguments.root, &arguments.staged)?;
        let source = arguments.staged.join("game");
        if !source.join("version.json").is_file() {
            return Err("staged release does not contain game/version.json".into());
        }

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let backup = arguments
            .root
            .join("Shroudforge_Updates/backups")
            .join(stamp.to_string());
        fs::create_dir_all(&backup).map_err(|error| error.to_string())?;
        append_log(
            &arguments.root,
            'I',
            &format!("Starting update from {}", arguments.staged.display()),
        );

        let result = apply(&source, &arguments.root, &backup);
        match result {
            Ok(()) => {
                let _ = fs::remove_file(arguments.root.join("Shroudforge_Updates/pending.ready"));
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
                if let Err(rollback_error) = restore(&backup, &arguments.root) {
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
        let updates = root.join("Shroudforge_Updates");
        if !staged.starts_with(&updates) {
            return Err("staged update is outside Shroudforge_Updates".into());
        }
        if root == staged {
            return Err("staged update must not equal install root".into());
        }
        Ok(())
    }

    fn apply(source: &Path, target: &Path, backup: &Path) -> Result<(), String> {
        for relative in MANAGED_PATHS {
            let current = target.join(relative);
            let saved = backup.join(relative);
            if current.exists() {
                copy_entry(&current, &saved)?;
            }
        }
        for relative in MANAGED_PATHS {
            let incoming = source.join(relative);
            if !incoming.exists() {
                return Err(format!("release is missing managed path: {relative}"));
            }
            let current = target.join(relative);
            remove_entry(&current)?;
            copy_entry(&incoming, &current)?;
        }
        Ok(())
    }

    fn restore(backup: &Path, target: &Path) -> Result<(), String> {
        for relative in MANAGED_PATHS {
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
        let path = root.join("Shroudforge_Updates/updater.log");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let elapsed = STARTED.get_or_init(Instant::now).elapsed();
            let total_seconds = elapsed.as_secs();
            let message = message.replace('\r', "\\r").replace('\n', "\\n");
            let _ = writeln!(
                file,
                "[{level} {:02}:{:02}:{:02},{:03}] [updater] {message}",
                total_seconds / 3600,
                total_seconds / 60 % 60,
                total_seconds % 60,
                elapsed.subsec_millis(),
            );
        }
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("ShroudForge update failed: {error}");
        std::process::exit(1);
    }
}
