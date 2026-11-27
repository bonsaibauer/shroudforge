use crate::mod_log_macros::{debug, warn};
use crate::mod_registry::ModInner;
use crate::path_types::{Path, PathBuf};
use crate::{
    FileSystem, IoError, Mod, ModEnvironmentErrorReport, ModError, ModErrorReport, ModRegistry,
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

impl ModRegistry {
    pub(crate) fn load(mods_dir: impl AsRef<Path>, game_dir: &Path) -> Result<Self, ModEnvironmentErrorReport> {
        let mods_dir = mods_dir.as_ref().to_path_buf();
        let mut mods = HashMap::new();

        if let Err(e) = std::fs::create_dir_all(&mods_dir) {
            return Err(ModEnvironmentErrorReport::base_io(IoError {
                path: mods_dir.into_string(),
                source: e,
            }));
        }

        let mod_files = match crate::modpack::package_locations(mods_dir.as_std_path()) {
            Ok(entries) => entries,
            Err(message) => {
                return Err(ModEnvironmentErrorReport::base_io(IoError {
                    path: mods_dir.into_string(),
                    source: std::io::Error::other(message),
                }));
            }
        };

        let mut errors = Vec::new();
        let mut candidates: HashMap<String, Vec<(Mod, PathBuf)>> = HashMap::new();

        for location in mod_files {
            if !location.pack_ready {
                warn!(pack = ?location.pack_id, "Skipping incomplete modpack");
                continue;
            }
            let path = match PathBuf::from_path_buf(location.path) {
                Ok(path) => path,
                Err(error) => {
                    warn!(path = %error.display(), "Skipping mod package with non-UTF-8 path");
                    continue;
                }
            };
            let r#mod = match Self::load_mod(&path, game_dir) {
                Ok(Some(m)) => m,
                Ok(None) => continue,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            candidates.entry(r#mod.info().id.clone()).or_default().push((r#mod, path));
        }

        for (id, copies) in candidates {
            let active = copies.iter().filter(|(item, _)| item.info().enabled).count();
            if active > 1 {
                errors.push(ModErrorReport::new(
                    copies[0].1.clone(),
                    ModError::DuplicateModId(id.clone()),
                ).with_id(id));
                continue;
            }
            let selected = if active == 1 {
                copies.into_iter().find(|(item, _)| item.info().enabled)
            } else {
                copies.into_iter().next()
            };
            if let Some((item, _)) = selected {
                mods.insert(id, item);
            }
        }
        let registry = Self { mods };

        if !errors.is_empty() {
            Err(ModEnvironmentErrorReport::with_mod_errors(errors, registry))
        } else {
            Ok(registry)
        }
    }

    fn load_mod(
        path: &PathBuf,
        root: &Path,
    ) -> Result<Option<Mod>, ModErrorReport> {
        let file_type = std::fs::symlink_metadata(path.as_std_path()).map_err(|e| {
            ModErrorReport::new(
                path.clone(),
                ModError::Io(IoError {
                    path: path.to_string(),
                    source: e,
                }),
            )
        })?.file_type();
        let file_name = path.file_name().unwrap_or_default();

        if file_name.starts_with('.') {
            debug!(path = path.as_str(), "Skipping hidden file or directory",);

            return Ok(None);
        }

        if !file_type.is_dir() || file_type.is_symlink() {
            debug!(path = path.as_str(), "Skipping non-package file");
            return Ok(None);
        }

        if !path.join("mod.json").exists() {
            debug!(
                path = path.as_str(),
                "Folder has no Lua manifest, native components are managed by the DLL Manager"
            );
            return Ok(None);
        }

        let mut fs = FileSystem::new_disk(path).map_err(|e| {
            ModErrorReport::new(
                path.clone(),
                ModError::Io(IoError {
                    path: path.to_string(),
                    source: e,
                }),
            )
        })?;

        let mod_info =
            crate::config::read_manifest(root.as_std_path(), &mut fs).map_err(|message| {
                ModErrorReport::new(
                    path.clone(),
                    ModError::Json {
                        path: path.join("mod.json").to_string(),
                        source: serde_json::Error::io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            message,
                        )),
                    },
                )
            })?;

        Ok(Some(Mod {
            inner: Arc::new(ModInner {
                info: mod_info,
                fs: Mutex::new(fs),
            }),
        }))
    }
}
