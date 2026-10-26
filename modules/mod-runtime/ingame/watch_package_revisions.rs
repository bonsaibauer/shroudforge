use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const REFRESH: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Shared {
    packages: Vec<PathBuf>,
    revisions: HashMap<PathBuf, Result<String, String>>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The revisions of the mods' packages (their mod.json and extended.mod.json),
/// read by a background thread every 500 ms: the in-game mods' loop read both
/// files of every mod each second to notice a settings change, and a read
/// could stall every mod for 100 ms at times. The thread ends with its owner.
pub(super) struct PackageRevisions {
    shared: Option<Arc<Mutex<Shared>>>,
}

impl PackageRevisions {
    pub(super) fn start() -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let watched = Arc::downgrade(&shared);
        let spawned = std::thread::Builder::new()
            .name("shroudforge-package-revisions".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(REFRESH);
                    let Some(shared) = watched.upgrade() else {
                        return;
                    };
                    let packages = lock(&shared).packages.clone();
                    let revisions: Vec<_> = packages
                        .into_iter()
                        .map(|package| {
                            let revision = mod_loader::config::manifest_revision(&package);
                            (package, revision)
                        })
                        .collect();
                    lock(&shared).revisions.extend(revisions);
                }
            });
        Self {
            shared: spawned.ok().map(|_| shared),
        }
    }

    /// The package's last revision read, or None until the thread has read it
    /// (the package is watched from its first call). Without a thread, read now.
    pub(super) fn revision(&self, package: &Path) -> Option<Result<String, String>> {
        let Some(shared) = &self.shared else {
            return Some(mod_loader::config::manifest_revision(package));
        };
        let mut shared = lock(shared);
        if !shared.packages.iter().any(|known| known == package) {
            shared.packages.push(package.to_path_buf());
        }
        shared.revisions.get(package).cloned()
    }
}
