use crate::ModEnvironment;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub fn fingerprint(env: &ModEnvironment, server: bool, api: &str) -> Result<String, String> {
    let root = env.game_dir().as_std_path();
    let mut hash = Sha256::new();
    hash.update(if server {
        b"server".as_slice()
    } else {
        b"client".as_slice()
    });
    let executable = root.join(if server {
        "enshrouded_server.exe"
    } else {
        "enshrouded.exe"
    });
    let metadata = fs::metadata(executable).map_err(|e| e.to_string())?;
    hash.update(metadata.len().to_le_bytes());
    hash.update(
        metadata
            .modified()
            .map_err(|e| e.to_string())?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    for item in env.plan(server, api) {
        if !item.info().requires_pregame() {
            continue;
        }
        hash.update(serde_json::to_vec(item.info()).map_err(|e| e.to_string())?);
        // Hash declared package files; backups and loader lock files are not inputs.
        {
            let path = item.fs().root().as_std_path().to_path_buf();
            if path.is_file() {
                hash.update(file_digest(&path)?);
            } else {
                let mut files = walkdir::WalkDir::new(&path)
                    .into_iter()
                    .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                files.sort_by_key(|entry| entry.path().to_path_buf());
                for entry in files
                    .into_iter()
                    .filter(|entry| entry.file_type().is_file() && !is_log_file(entry.path()))
                {
                    hash.update(
                        entry
                            .path()
                            .strip_prefix(&path)
                            .map_err(|e| e.to_string())?
                            .to_string_lossy()
                            .as_bytes(),
                    );
                    hash.update(file_digest(entry.path())?);
                }
            }
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn matches(root: &Path, fingerprint: &str) -> bool {
    // The KFC rewrite is persistent. A matching record confirms that the
    // installed asset data already reflects this game and enabled mod plan.
    crate::config::read_document(root, shroudforge_game_assets::documents::APPLIED)
        .ok()
        .is_some_and(|value| value["status"] == "applied" && value["fingerprint"] == fingerprint)
}
/// Logs are output, not input. Native mod DLLs such as Hands-Free Lantern
/// write `*.log` files into their own package folder after the startup pass,
/// which would otherwise invalidate the applied record on every game start.
fn is_log_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("log"))
}

fn file_digest(path: &Path) -> Result<Vec<u8>, String> {
    use std::{
        collections::HashMap,
        io::Read,
        sync::{Mutex, OnceLock},
        time::SystemTime,
    };
    type Entry = (u64, SystemTime, Vec<u8>);
    static CACHE: OnceLock<Mutex<HashMap<std::path::PathBuf, Entry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let stamp = metadata.modified().map_err(|e| e.to_string())?;
    if let Ok(cache) = cache.lock() {
        if let Some((length, time, digest)) = cache.get(path) {
            if *length == metadata.len() && *time == stamp {
                return Ok(digest.clone());
            }
        }
    }
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let digest = hash.finalize().to_vec();
    if let Ok(mut cache) = cache.lock() {
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(path.to_path_buf(), (metadata.len(), stamp, digest.clone()));
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::fingerprint;

    #[test]
    fn mod_log_files_do_not_change_the_preparation_fingerprint() {
        let root = std::env::temp_dir().join(format!(
            "sf-prepared-fingerprint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let package = root.join("mods").join("lantern");
        std::fs::create_dir_all(package.join("src")).unwrap();
        std::fs::write(
            package.join("mod.json"),
            r#"{"id":"lantern","name":"lantern","version":"1.0.0","capabilities":["patch"]}"#,
        )
        .unwrap();
        std::fs::write(
            package.join("extended.mod.json"),
            r#"{"schemaVersion":1,"enabled":true,"targets":["client","server"]}"#,
        )
        .unwrap();
        std::fs::write(package.join("src/mod.lua"), "-- patch").unwrap();
        std::fs::write(root.join("enshrouded.exe"), b"test executable identity").unwrap();
        shroudforge_loader_config::config::initialize_loader_config(&root).unwrap();
        let read = || {
            let environment = crate::ModEnvironment::load(root.to_str().unwrap()).unwrap();
            fingerprint(&environment, false, "1.0.0").unwrap()
        };
        let prepared = read();

        std::fs::write(
            package.join("hfl-lantern.log"),
            b"written by the native DLL",
        )
        .unwrap();
        std::fs::write(package.join("Session.LOG"), b"case does not matter").unwrap();
        assert_eq!(read(), prepared);

        std::fs::write(package.join("src/mod.lua"), "-- changed patch").unwrap();
        assert_ne!(read(), prepared);
    }
}
