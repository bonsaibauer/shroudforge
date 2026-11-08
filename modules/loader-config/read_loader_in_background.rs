use crate::config::read_loader;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

type Cached = Arc<Mutex<Result<Value, String>>>;

const REFRESH: Duration = Duration::from_millis(250);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The loader settings as a background thread last read them, at most
/// 250 ms ago, for callers that must not wait on the disk: the in-game mods'
/// loop read them several times a second, and a read could stall every mod
/// for 100 ms at times. The first call for a root reads them at once.
/// Without a thread, every call reads them.
pub fn read_loader_in_background(root: &Path) -> Result<Value, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<Cached>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut roots = lock(cache);
    let entry = roots.entry(root.to_path_buf()).or_insert_with(|| {
        let cached: Cached = Arc::new(Mutex::new(read_loader(root)));
        let shared = cached.clone();
        let path = root.to_path_buf();
        std::thread::Builder::new()
            .name("shroudforge-loader-settings".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(REFRESH);
                    let value = read_loader(&path);
                    *lock(&shared) = value;
                }
            })
            .ok()
            .map(|_| cached)
    });
    match entry {
        Some(cached) => lock(cached).clone(),
        None => read_loader(root),
    }
}
