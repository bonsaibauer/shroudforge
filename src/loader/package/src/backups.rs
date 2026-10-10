//! Persistent game-file originals and verified restoration.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn stem(target: &str) -> Result<&'static str, String> {
    match target {
        "client" | "enshrouded" => Ok("enshrouded"),
        "server" | "enshrouded_server" => Ok("enshrouded_server"),
        _ => Err("unknown game target".into()),
    }
}

fn target_name(stem: &str) -> &'static str {
    if stem == "enshrouded_server" {
        "server"
    } else {
        "client"
    }
}

fn pair(stem: &str) -> [String; 2] {
    [format!("{stem}.kfc"), format!("{stem}.kfc_resources")]
}

fn game_executable(root: &Path, stem: &str) -> PathBuf {
    root.join(if stem == "enshrouded_server" {
        "enshrouded_server.exe"
    } else {
        "enshrouded.exe"
    })
}

fn originals_target_dir(root: &Path, stem: &str) -> PathBuf {
    crate::paths::backups_dir(root)
        .join("originals")
        .join(target_name(stem))
}

fn originals_dir(root: &Path, stem: &str) -> PathBuf {
    let key = fs::metadata(game_executable(root, stem))
        .ok()
        .and_then(|metadata| {
            Some((
                metadata.len(),
                metadata
                    .modified()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .ok()?
                    .as_secs(),
            ))
        })
        .map(|(size, modified)| format!("{size}-{modified}"))
        .unwrap_or_else(|| "unknown-build".into());
    originals_target_dir(root, stem).join(key)
}

fn sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn manifest(root: &Path, stem: &str) -> Result<Value, String> {
    let dir = originals_dir(root, stem);
    let value: Value = serde_json::from_slice(
        &fs::read(dir.join("manifest.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("invalid original-backup manifest: {error}"))?;
    if value["schemaVersion"] != 1 || value["target"] != target_name(stem) {
        return Err("original-backup manifest does not match this game target".into());
    }
    Ok(value)
}

pub fn originals_status(root: &Path, target: &str) -> Value {
    let Ok(stem) = stem(target) else {
        return json!({"target":target,"status":"unsupported","files":[]});
    };
    let dir = originals_dir(root, stem);
    if !dir.exists() {
        let files = pair(stem);
        let ambiguous = files
            .iter()
            .any(|name| root.join(format!("{name}.bak")).exists())
            || crate::config::read_document(root, "applied")
                .ok()
                .is_some_and(|value| {
                    matches!(
                        value["status"].as_str(),
                        Some("applied" | "preparing" | "failed")
                    )
                });
        let old_builds = fs::read_dir(originals_target_dir(root, stem))
            .ok()
            .is_some_and(|mut entries| entries.next().is_some());
        return json!({"target":target_name(stem),"status":if ambiguous || old_builds {"verification-required"} else {"missing"},"files":[]});
    }
    let value = match manifest(root, stem) {
        Ok(value) => value,
        Err(error) => {
            return json!({"target":target_name(stem),"status":"invalid","detail":error,"files":[]});
        }
    };
    let Some(entries) = value["files"].as_array() else {
        return json!({"target":target_name(stem),"status":"invalid","detail":"manifest has no file list","files":[]});
    };
    let mut valid = entries.len() == 2;
    let mut found = std::collections::HashSet::new();
    let files: Vec<Value> = entries
        .iter()
        .map(|entry| {
            let name = entry["name"].as_str().unwrap_or_default();
            if !pair(stem).iter().any(|expected| expected == name) {
                valid = false;
                return json!({"name":name,"valid":false});
            }
            if !found.insert(name.to_owned()) {
                valid = false;
                return json!({"name":name,"valid":false});
            }
            let path = dir.join(name);
            let actual_size = fs::metadata(&path).ok().map(|metadata| metadata.len());
            let expected = entry["sha256"].as_str().unwrap_or_default();
            let expected_size = entry["size"].as_u64();
            // Full checksums are verified in the updater before restoration. Avoid
            // hashing potentially large resources files during every UI refresh.
            let ok = actual_size.is_some() && actual_size == expected_size && expected.len() == 64;
            valid &= ok;
            json!({"name":name,"size":entry["size"],"valid":ok})
        })
        .collect();
    valid &= pair(stem).iter().all(|name| found.contains(name));
    json!({"target":target_name(stem),"status":if valid {"ready"} else {"invalid"},"capturedAt":value["capturedAt"],"source":value["source"],"gameExecutable":value["gameExecutable"],"files":files})
}

/// One read-only inventory for the UI. Working `.bak` files and short-lived
/// transactions are listed separately from immutable installation originals.
pub fn management_status(root: &Path, target: &str) -> Value {
    let Ok(stem) = stem(target) else {
        return json!({"target":target,"originals":{"status":"unsupported"}});
    };
    let kfc_backup = root.join(format!("{stem}.kfc.bak"));
    let resources_backup = root.join(format!("{stem}.kfc_resources.bak"));
    let transaction = root.join(format!(".{stem}.shroudforge-stage"));
    let restore_status_path = crate::paths::backups_dir(root).join("gamefiles-restore-status.json");
    let restore = fs::read(&restore_status_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .unwrap_or_else(|| json!({"status":"idle"}));
    let count_dirs = |path: PathBuf| -> usize {
        fs::read_dir(path)
            .ok()
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| entry.path().is_dir())
                    .count()
            })
            .unwrap_or(0)
    };
    json!({
        "target":target_name(stem),
        "originals":originals_status(root, stem),
        "workingBaseline":{"kfc":kfc_backup.is_file(),"resources":resources_backup.is_file(),"complete":kfc_backup.is_file()&&resources_backup.is_file()},
        "transaction":{"pending":transaction.join("pending.json").is_file()},
        "rollback":{"systemUpdates":count_dirs(crate::paths::updates_dir(root).join("backups")),"modPackages":count_dirs(crate::paths::updates_dir(root).join("mod-backups"))},
        "restore":restore
    })
}

/// Captures the files only when no legacy working backup or prior application
/// indicates they may already have been modified. Existing originals are immutable.
pub fn ensure_originals(root: &Path, target: &str) -> Result<Value, String> {
    let stem = stem(target)?;
    let status = originals_status(root, stem);
    if status["status"] != "missing" {
        return Ok(status);
    }
    capture_originals_inner(root, stem, false)?;
    Ok(originals_status(root, stem))
}

/// Explicit capture is intended for users who have verified the game files.
/// It still refuses to replace an existing originals directory.
pub fn capture_verified_originals(root: &Path, target: &str) -> Result<Value, String> {
    let stem = stem(target)?;
    if originals_dir(root, stem).exists() {
        return Err("an original-backup set already exists and will not be overwritten".into());
    }
    capture_originals_inner(root, stem, true)?;
    Ok(originals_status(root, stem))
}

fn capture_originals_inner(root: &Path, stem: &str, user_verified: bool) -> Result<(), String> {
    let status = originals_status(root, stem);
    if !user_verified && status["status"] != "missing" {
        return Err("cannot automatically mark an uncertain game-file set as original".into());
    }
    if !user_verified {
        let legacy_backup = pair(stem)
            .iter()
            .any(|name| root.join(format!("{name}.bak")).exists());
        let previous_apply = crate::config::read_document(root, "applied")
            .ok()
            .is_some_and(|value| {
                matches!(
                    value["status"].as_str(),
                    Some("applied" | "preparing" | "failed")
                )
            });
        if legacy_backup || previous_apply {
            return Err("existing mod or backup state makes the clean origin uncertain".into());
        }
    }
    let files = pair(stem);
    for name in &files {
        if !root.join(name).is_file() {
            return Err(format!("required game file is missing: {name}"));
        }
    }
    let exe_name = if stem == "enshrouded_server" {
        "enshrouded_server.exe"
    } else {
        "enshrouded.exe"
    };
    let exe = game_executable(root, stem);
    let exe_meta = fs::metadata(&exe).map_err(|error| format!("{}: {error}", exe.display()))?;
    let exe_modified = exe_meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|time| time.as_secs());
    let final_dir = originals_dir(root, stem);
    let parent = final_dir
        .parent()
        .ok_or("invalid originals directory")?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stage = parent.join(format!(".capture-{}-{nonce}", std::process::id()));
    fs::create_dir(&stage).map_err(|error| error.to_string())?;
    let result = (|| {
        let mut entries = Vec::new();
        for name in &files {
            let source = root.join(name);
            let target = stage.join(name);
            fs::copy(&source, &target).map_err(|error| error.to_string())?;
            let copied_hash = sha256(&target)?;
            if copied_hash != sha256(&source)? {
                return Err(format!(
                    "game file changed while its original was being captured: {name}"
                ));
            }
            let size = fs::metadata(&target)
                .map_err(|error| error.to_string())?
                .len();
            entries.push(json!({"name":name,"size":size,"sha256":copied_hash}));
        }
        let game_executable = json!({"name":exe_name,"size":exe_meta.len(),"modified":exe_modified,"sha256":sha256(&exe)?});
        let metadata = json!({
            "schemaVersion":1,
            "target":target_name(stem),
            "source":if user_verified {"user-verified"} else {"first-managed-start"},
            "capturedAt":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            "gameExecutable":game_executable,
            "files":entries
        });
        crate::config::write_json(&stage.join("manifest.json"), &metadata)?;
        match fs::rename(&stage, &final_dir) {
            Ok(()) => Ok(()),
            Err(_error) if final_dir.exists() => Err(
                "Original backup set appeared concurrently. Existing originals were preserved"
                    .into(),
            ),
            Err(error) => Err(error.to_string()),
        }
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

/// Restores the verified immutable pair and retains a recovery copy of the
/// current pair in case either replacement fails.
pub fn restore_originals(root: &Path, target: &str) -> Result<Value, String> {
    let stem = stem(target)?;
    let status = originals_status(root, stem);
    if status["status"] != "ready" {
        return Err("verified original game files are not available".into());
    }
    let manifest = manifest(root, stem)?;
    let executable_hash = manifest["gameExecutable"]["sha256"]
        .as_str()
        .ok_or("original backup does not identify its Enshrouded build")?;
    if sha256(&game_executable(root, stem))? != executable_hash {
        return Err(
            "saved original files belong to a different Enshrouded executable build".into(),
        );
    }
    let dir = originals_dir(root, stem);
    let names = pair(stem);
    let restore_root = crate::paths::backups_dir(root).join("restore-points");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let safety = restore_root.join(format!(
        "{}-{stamp}-{}",
        target_name(stem),
        std::process::id()
    ));
    fs::create_dir(&safety).map_err(|error| error.to_string())?;
    for name in &names {
        if !root.join(name).is_file() {
            return Err(format!("current game file is missing: {name}"));
        }
        fs::copy(root.join(name), safety.join(name)).map_err(|error| error.to_string())?;
    }
    let mut prepared = Vec::new();
    for name in &names {
        let expected = manifest["files"]
            .as_array()
            .and_then(|files| files.iter().find(|file| file["name"] == *name))
            .and_then(|file| file["sha256"].as_str())
            .ok_or("original manifest is incomplete")?;
        let temp = root.join(format!(".{name}.restore-{}", std::process::id()));
        fs::copy(dir.join(name), &temp).map_err(|error| error.to_string())?;
        if sha256(&temp)? != expected {
            let _ = fs::remove_file(&temp);
            return Err(format!("original backup failed hash verification: {name}"));
        }
        prepared.push((name.clone(), temp));
    }
    let mut replaced: Vec<String> = Vec::new();
    for (name, temp) in &prepared {
        let target_path = root.join(name);
        let displaced = root.join(format!(".{name}.restore-old-{}", std::process::id()));
        let _ = fs::remove_file(&displaced);
        if let Err(error) = fs::rename(&target_path, &displaced) {
            rollback_files(root, &safety, &replaced);
            return Err(format!("could not stage current {name}: {error}"));
        }
        if let Err(error) = fs::rename(temp, &target_path) {
            let _ = fs::rename(&displaced, &target_path);
            rollback_files(root, &safety, &replaced);
            return Err(format!("could not install original {name}: {error}"));
        }
        let _ = fs::remove_file(displaced);
        replaced.push(name.clone());
        let expected = manifest["files"]
            .as_array()
            .and_then(|files| files.iter().find(|file| file["name"] == *name))
            .and_then(|file| file["sha256"].as_str())
            .unwrap_or_default();
        if sha256(&target_path).as_deref() != Ok(expected) {
            rollback_files(root, &safety, &replaced);
            return Err(format!("restored file failed hash verification: {name}"));
        }
    }
    Ok(
        json!({"status":"restored","target":target_name(stem),"safetyBackup":safety.display().to_string(),"files":names}),
    )
}

fn rollback_files(root: &Path, safety: &Path, replaced: &[String]) {
    for name in replaced.iter().rev() {
        let source = safety.join(name);
        let target = root.join(name);
        let temp = root.join(format!(".{name}.rollback-{}", std::process::id()));
        if fs::copy(source, &temp).is_ok() {
            let displaced = root.join(format!(".{name}.failed-{}", std::process::id()));
            let _ = fs::rename(&target, &displaced);
            if fs::rename(&temp, &target).is_ok() {
                let _ = fs::remove_file(displaced);
            }
        }
    }
}
