use super::{
    game_executable, manifest, originals_dir, originals_status, pair, sha256, stem, target_name,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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

/// Restore the verified pair through the same journal used by startup asset
/// publication. The caller owns the startup lease and invalidates the applied
/// fingerprint before invoking this operation.
pub fn restore_originals_for_startup(root: &Path, target: &str) -> Result<(), String> {
    let stem = stem(target)?;
    if originals_status(root, stem)["status"] != "ready" {
        return Err("verified original game files are not available for this game build".into());
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
    let names = pair(stem);
    let mut expected = Vec::new();
    for name in &names {
        let hash = manifest["files"]
            .as_array()
            .and_then(|files| files.iter().find(|file| file["name"] == *name))
            .and_then(|file| file["sha256"].as_str())
            .ok_or("original manifest is incomplete")?;
        if sha256(&originals_dir(root, stem).join(name))? != hash {
            return Err(format!("original backup failed hash verification: {name}"));
        }
        expected.push(hash.to_owned());
    }
    if names
        .iter()
        .zip(&expected)
        .all(|(name, hash)| sha256(&root.join(name)).as_deref() == Ok(hash))
    {
        return Ok(());
    }
    let stage = crate::transaction::begin(root, stem).map_err(|error| error.to_string())?;
    let result = (|| {
        for (name, hash) in names.iter().zip(&expected) {
            let target = stage.join(name);
            fs::copy(originals_dir(root, stem).join(name), &target)
                .map_err(|error| error.to_string())?;
            if sha256(&target)? != *hash {
                return Err(format!("staged original failed hash verification: {name}"));
            }
        }
        crate::transaction::commit(root, stem).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        crate::transaction::abort(root, stem).map_err(|error| error.to_string())?;
    }
    result
}
