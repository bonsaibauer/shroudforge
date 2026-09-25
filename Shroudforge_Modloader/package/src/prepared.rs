use crate::{Capability, ModEnvironment};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub fn fingerprint(env: &ModEnvironment, server: bool, api: &str) -> Result<String, String> {
    let root = env.game_dir().as_std_path();
    let mut hash = Sha256::new();
    hash.update(if server { b"server".as_slice() } else { b"client".as_slice() });
    let executable = root.join(if server { "enshrouded_server.exe" } else { "enshrouded.exe" });
    let metadata = fs::metadata(executable).map_err(|e| e.to_string())?;
    hash.update(metadata.len().to_le_bytes());
    hash.update(metadata.modified().map_err(|e| e.to_string())?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos().to_le_bytes());
    for item in env.plan(server, api) {
        if !item.info().capabilities.iter().any(|c| matches!(c, Capability::AssetsWrite | Capability::Export)) { continue; }
        hash.update(serde_json::to_vec(item.info()).map_err(|e| e.to_string())?);
        // Hash declared package files; backups and loader lock files are not inputs.
        {
            let path = item.fs().root().as_std_path().to_path_buf();
            if path.is_file() { hash.update(file_digest(&path)?); }
            else {
                let mut files = walkdir::WalkDir::new(&path).into_iter()
                    .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
                    .collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
                files.sort_by_key(|entry| entry.path().to_path_buf());
                for entry in files.into_iter().filter(|entry| entry.file_type().is_file()) {
                    hash.update(entry.path().strip_prefix(&path).map_err(|e| e.to_string())?.to_string_lossy().as_bytes());
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
    crate::config::read_document(root, "applied").ok()
        .is_some_and(|value| value["status"] == "applied" && value["fingerprint"] == fingerprint)
}
fn file_digest(path: &Path) -> Result<Vec<u8>,String> {
    use std::{collections::HashMap,io::Read,sync::{Mutex,OnceLock},time::SystemTime};
    type Entry=(u64,SystemTime,Vec<u8>);
    static CACHE: OnceLock<Mutex<HashMap<std::path::PathBuf,Entry>>>=OnceLock::new();
    let cache=CACHE.get_or_init(||Mutex::new(HashMap::new()));
    let metadata=fs::metadata(path).map_err(|e|e.to_string())?;
    let stamp=metadata.modified().map_err(|e|e.to_string())?;
    if let Ok(cache)=cache.lock() {
        if let Some((length,time,digest))=cache.get(path) {
            if *length==metadata.len() && *time==stamp {return Ok(digest.clone());}
        }
    }
    let mut file=fs::File::open(path).map_err(|e|e.to_string())?;
    let mut hash=Sha256::new();
    let mut buffer=[0u8;65536];
    loop {let count=file.read(&mut buffer).map_err(|e|e.to_string())?;if count==0{break;}hash.update(&buffer[..count]);}
    let digest=hash.finalize().to_vec();
    if let Ok(mut cache)=cache.lock() {
        if cache.len()>4096{cache.clear();}
        cache.insert(path.to_path_buf(),(metadata.len(),stamp,digest.clone()));
    }
    Ok(digest)
}
