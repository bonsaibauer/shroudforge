use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const CACHE_FILE: &str = "shroudedit-api-cache.json";
const LOCK_FILE: &str = "shroudedit-api-cache.lock";
const CACHE_SCHEMA_VERSION: u32 = 1;
const MAX_CACHE_ENTRIES: usize = 4096;
const MAX_CACHE_CHARACTERS: usize = 1_500_000;
const MAX_CACHE_FILE_BYTES: u64 = 3_000_000;

static PROCESS_CACHE: OnceLock<Mutex<Option<CacheSnapshot>>> = OnceLock::new();

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
struct CacheDocument {
    schema_version: u32,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheEntry {
    saved_at: u64,
    value: Value,
}

struct CacheSnapshot {
    path: PathBuf,
    modified: Option<SystemTime>,
    length: u64,
    document: CacheDocument,
}

pub(super) fn key(namespace: &str, provider: &str, dimensions: &Value) -> String {
    serde_json::to_string(&serde_json::json!([
        CACHE_SCHEMA_VERSION,
        provider.trim_end_matches('/'),
        namespace,
        dimensions
    ]))
    .unwrap_or_default()
}

pub(super) fn read<T: DeserializeOwned>(
    root: &std::path::Path,
    key: &str,
    max_age_seconds: u64,
) -> Option<T> {
    let (mut process_cache, _file_guard) = lock_cache(root)?;
    let path = cache_path(root);
    let cache = cached_document(&mut process_cache, &path);
    let entry = cache.entries.get(key)?;
    if unix_seconds().saturating_sub(entry.saved_at) >= max_age_seconds {
        return None;
    }
    serde_json::from_value(entry.value.clone()).ok()
}

pub(super) fn write<T: Serialize>(root: &std::path::Path, key: String, value: &T) {
    let Ok(value) = serde_json::to_value(value) else {
        return;
    };
    let entry = CacheEntry {
        saved_at: unix_seconds(),
        value,
    };
    let Ok(entry_value) = serde_json::to_value(&entry) else {
        return;
    };
    if serialized_characters(&entry_value) > MAX_CACHE_CHARACTERS {
        return;
    }
    let Some((mut process_cache, _file_guard)) = lock_cache(root) else {
        return;
    };
    let path = cache_path(root);
    let mut cache = cached_document(&mut process_cache, &path).clone();
    cache.schema_version = CACHE_SCHEMA_VERSION;
    cache.entries.insert(key.clone(), entry);
    trim_cache(&mut cache, &key);
    let Ok(document) = serde_json::to_value(&cache) else {
        return;
    };
    if serialized_characters(&document) > MAX_CACHE_CHARACTERS
        || serde_json::to_vec_pretty(&document)
            .map(|bytes| bytes.len() as u64 > MAX_CACHE_FILE_BYTES)
            .unwrap_or(true)
    {
        return;
    }
    if shroudforge_loader_config::config::write_json(&path, &document).is_ok() {
        let (length, modified) = file_stamp(&path);
        *process_cache = Some(CacheSnapshot {
            path,
            modified,
            length,
            document: cache,
        });
    }
}

pub(super) fn clear(root: &std::path::Path) -> Result<(), String> {
    let Some((mut process_cache, _file_guard)) = lock_cache(root) else {
        return Err("Could not access the updater API cache.".into());
    };
    match fs::remove_file(cache_path(root)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Could not delete the updater API cache: {error}")),
    }
    *process_cache = None;
    Ok(())
}

fn lock_cache(
    root: &std::path::Path,
) -> Option<(MutexGuard<'static, Option<CacheSnapshot>>, File)> {
    let process_guard = PROCESS_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let directory = shroudforge_loader_config::paths::cache_dir(root);
    fs::create_dir_all(&directory).ok()?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(directory.join(LOCK_FILE))
        .ok()?;
    fs2::FileExt::lock_exclusive(&file).ok()?;
    Some((process_guard, file))
}

fn cache_path(root: &std::path::Path) -> PathBuf {
    shroudforge_loader_config::paths::cache_dir(root).join(CACHE_FILE)
}

fn read_document(path: &std::path::Path) -> CacheDocument {
    let Ok(metadata) = fs::metadata(path) else {
        return CacheDocument::default();
    };
    if metadata.len() > MAX_CACHE_FILE_BYTES {
        return CacheDocument::default();
    }
    let Ok(bytes) = fs::read(path) else {
        return CacheDocument::default();
    };
    serde_json::from_slice::<CacheDocument>(&bytes)
        .ok()
        .filter(|cache| cache.schema_version == CACHE_SCHEMA_VERSION)
        .unwrap_or_default()
}

fn cached_document<'a>(
    cached: &'a mut Option<CacheSnapshot>,
    path: &std::path::Path,
) -> &'a CacheDocument {
    let (length, modified) = file_stamp(path);
    if !cached.as_ref().is_some_and(|snapshot| {
        snapshot.path == path
            && snapshot.length == length
            && snapshot.modified == modified
    }) {
        *cached = Some(CacheSnapshot {
            path: path.to_path_buf(),
            modified,
            length,
            document: read_document(path),
        });
    }
    &cached.as_ref().expect("cache snapshot was initialized").document
}

fn file_stamp(path: &std::path::Path) -> (u64, Option<SystemTime>) {
    fs::metadata(path)
        .map(|metadata| (metadata.len(), metadata.modified().ok()))
        .unwrap_or_default()
}

fn trim_cache(cache: &mut CacheDocument, protected_key: &str) {
    while cache.entries.len() > MAX_CACHE_ENTRIES
        || serialized_characters(&serde_json::to_value(&*cache).unwrap_or(Value::Null))
            > MAX_CACHE_CHARACTERS
    {
        let oldest = cache
            .entries
            .iter()
            .filter(|(key, _)| key.as_str() != protected_key)
            .min_by_key(|(_, entry)| entry.saved_at)
            .map(|(key, _)| key.clone());
        let Some(oldest) = oldest else {
            cache.entries.remove(protected_key);
            return;
        };
        cache.entries.remove(&oldest);
    }
}

fn serialized_characters(value: &Value) -> usize {
    serde_json::to_string(value)
        .map(|serialized| serialized.chars().count())
        .unwrap_or(usize::MAX)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
