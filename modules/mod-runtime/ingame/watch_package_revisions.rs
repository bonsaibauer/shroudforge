use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const REFRESH: Duration = Duration::from_millis(500);

type Manifest = Result<mod_loader::ModManifest, String>;

#[derive(Default)]
struct Shared {
    packages: Vec<PathBuf>,
    revisions: HashMap<PathBuf, Result<String, String>>,
    // The manifest read for a revision, ready when the runtime applies it.
    manifests: HashMap<PathBuf, (String, Manifest)>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The revisions of the mods' packages (their mod.json and extended.mod.json),
/// read by a background thread every 500 ms, with the manifest of each new
/// revision: the in-game mods' loop read both files of every mod each second
/// to notice a settings change, and a read could stall every mod for 100 ms
/// at times. The thread ends with its owner.
pub(super) struct PackageRevisions {
    shared: Option<Arc<Mutex<Shared>>>,
}

impl PackageRevisions {
    pub(super) fn start(root: PathBuf) -> Self {
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
                    let (packages, read) = {
                        let shared = lock(&shared);
                        let read: HashMap<PathBuf, String> = shared
                            .manifests
                            .iter()
                            .map(|(package, (revision, _))| (package.clone(), revision.clone()))
                            .collect();
                        (shared.packages.clone(), read)
                    };
                    let mut revisions = Vec::new();
                    let mut manifests = Vec::new();
                    for package in packages {
                        let revision = mod_loader::config::manifest_revision(&package);
                        if let Ok(revision) = &revision {
                            if read.get(&package) != Some(revision) {
                                let manifest =
                                    mod_loader::config::read_manifest_path(&root, &package);
                                manifests.push((package.clone(), (revision.clone(), manifest)));
                            }
                        }
                        revisions.push((package, revision));
                    }
                    let mut shared = lock(&shared);
                    shared.revisions.extend(revisions);
                    shared.manifests.extend(manifests);
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

    /// The package's manifest read for `revision`, or None when the thread
    /// has not read that one (the caller then reads it).
    pub(super) fn manifest(&self, package: &Path, revision: &str) -> Option<Manifest> {
        let shared = lock(self.shared.as_ref()?);
        match shared.manifests.get(package) {
            Some((read, manifest)) if read == revision => Some(manifest.clone()),
            _ => None,
        }
    }
}
