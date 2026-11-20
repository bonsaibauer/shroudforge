//! Persistent game-file originals and verified restoration.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

pub(crate) fn stem(target: &str) -> Result<&'static str, String> {
    match target {
        "client" | "enshrouded" => Ok("enshrouded"),
        "server" | "enshrouded_server" => Ok("enshrouded_server"),
        _ => Err("unknown game target".into()),
    }
}

pub(crate) fn target_name(stem: &str) -> &'static str {
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

mod capture_original_game_files;
mod read_original_backups;
mod restore_original_game_files;

pub use crate::read_game_file_status::management_status;
pub use capture_original_game_files::{capture_verified_originals, ensure_originals};
pub use read_original_backups::originals_status;
pub use restore_original_game_files::{restore_originals, restore_originals_for_startup};
