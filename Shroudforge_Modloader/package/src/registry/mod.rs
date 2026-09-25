use std::{collections::HashMap, fs::DirEntry, ops::Deref, sync::Arc};

mod fs;
mod manifest;

use crate::{
    IoError, ModEnvironmentErrorReport, ModError, ModErrorReport,
    alias::{Path, PathBuf},
    log::{info, warn},
};
use parking_lot::{Mutex, MutexGuard};

pub use fs::FileSystem;
pub use manifest::*;

#[derive(Debug)]
struct ModInner {
    info: ModManifest,
    fs: Mutex<FileSystem>,
}

#[derive(Debug, Clone)]
pub struct Mod {
    inner: Arc<ModInner>,
}

impl Mod {
    pub fn info(&self) -> &ModManifest {
        &self.inner.info
    }

    pub fn fs(&self) -> MutexGuard<'_, FileSystem> {
        self.inner.fs.lock()
    }
}

#[derive(Debug, Default)]
pub struct ModRegistry {
    mods: HashMap<String, Mod>,
}

impl ModRegistry {
    pub(crate) fn load(mods_dir: impl AsRef<Path>) -> Result<Self, ModEnvironmentErrorReport> {
        let mods_dir = mods_dir.as_ref().to_path_buf();
        let mut mods = HashMap::new();

        if let Err(e) = std::fs::create_dir_all(&mods_dir) {
            return Err(ModEnvironmentErrorReport::base_io(IoError {
                path: mods_dir.into_string(),
                source: e,
            }));
        }

        let mod_files = match std::fs::read_dir(&mods_dir) {
            Ok(entries) => entries,
            Err(e) => {
                return Err(ModEnvironmentErrorReport::base_io(IoError {
                    path: mods_dir.into_string(),
                    source: e,
                }));
            }
        };

        let mut errors = Vec::new();
        let mut duplicate_ids = std::collections::HashSet::new();

        for entry in mod_files {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!(
                        error = %e,
                        path = mods_dir.as_str(),
                        "Error reading entry in mods directory, skipping",
                    );

                    continue;
                }
            };

            let r#mod = match Self::load_mod(&mods, entry, mods_dir.parent().unwrap_or(&mods_dir)) {
                Ok(Some(m)) => m,
                Ok(None) => continue,
                Err(e) => {
                    if let ModError::DuplicateModId(id) = &e.error {
                        duplicate_ids.insert(id.clone());
                    }
                    errors.push(e);
                    continue;
                }
            };

            let mod_id = r#mod.info().id.clone();

            mods.insert(mod_id, r#mod);
        }

        for id in duplicate_ids {
            mods.remove(&id);
        }
        let registry = Self { mods };

        if !errors.is_empty() {
            Err(ModEnvironmentErrorReport::with_mod_errors(errors, registry))
        } else {
            Ok(registry)
        }
    }

    fn load_mod(
        mods: &HashMap<String, Mod>,
        entry: DirEntry,
        root: &Path,
    ) -> Result<Option<Mod>, ModErrorReport> {
        let path = PathBuf::from_path_buf(entry.path()).map_err(|e| {
            ModErrorReport::new(
                entry.path().to_string_lossy().to_string(),
                ModError::Utf8(e),
            )
        })?;

        let file_type = entry.file_type().map_err(|e| {
            ModErrorReport::new(
                path.clone(),
                ModError::Io(IoError {
                    path: path.to_string(),
                    source: e,
                }),
            )
        })?;
        let file_name = path.file_name().unwrap_or_default();

        if file_name.starts_with('.') {
            info!(path = path.as_str(), "Skipping hidden file or directory",);

            return Ok(None);
        }

        let mut fs = if file_type.is_dir() {
            FileSystem::new_disk(&path).map_err(|e| {
                ModErrorReport::new(
                    path.clone(),
                    ModError::Io(IoError {
                        path: path.to_string(),
                        source: e,
                    }),
                )
            })?
        } else if file_type.is_file() {
            let extension = path.extension().unwrap_or_default();

            match extension {
                "zip" => {}
                _ => return Ok(None),
            }

            FileSystem::new_zip(&path).map_err(|e| {
                ModErrorReport::new(
                    path.clone(),
                    ModError::Io(IoError {
                        path: path.to_string(),
                        source: e,
                    }),
                )
            })?
        } else {
            warn!(
                path = path.as_str(),
                "Skipping non-file and non-directory entry"
            );

            return Ok(None);
        };

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

        let mod_id = mod_info.id.clone();

        if mods.contains_key(&mod_id) {
            return Err(ModErrorReport::new(
                path.clone(),
                ModError::DuplicateModId(mod_id.clone()),
            )
            .with_id(mod_id));
        }

        Ok(Some(Mod {
            inner: Arc::new(ModInner {
                info: mod_info,
                fs: Mutex::new(fs),
            }),
        }))
    }
}

/// Validates the public package contract without loading or executing a mod.
pub fn validate_manifest(manifest: &ModManifest) -> Result<(), String> {
    fn identifier(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 80
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    }

    if !crate::config::valid_id(&manifest.id) {
        return Err("mod id is invalid".into());
    }
    let configured_link_ids: Vec<String> = serde_json::from_str(include_str!("../../../../config/links/order.json"))
        .map_err(|error| format!("invalid configured mod link registry: {error}"))?;
    for (name, url) in &manifest.links.0 {
        if !configured_link_ids.iter().any(|id| id == name) {
            return Err(format!("unsupported mod link '{name}'"));
        }
        if !url.starts_with("https://") {
            return Err(format!("mod link '{name}' must use HTTPS"));
        }
    }
    let mut setting_keys = std::collections::HashSet::new();
    for setting in &manifest.settings {
        if !identifier(&setting.key) || !setting_keys.insert(setting.key.as_str()) {
            return Err(format!(
                "setting key '{}' is invalid or duplicated",
                setting.key
            ));
        }
        let valid_default = match setting.value_type {
            SettingValueType::Boolean => setting.default.is_boolean(),
            SettingValueType::String => setting.default.is_string(),
            SettingValueType::Integer => setting.default.as_i64().is_some(),
            SettingValueType::Number => setting.default.is_number(),
            SettingValueType::Array => setting.default.is_array(),
        };
        if !valid_default {
            return Err(format!(
                "setting '{}' has an invalid default value",
                setting.key
            ));
        }
        if let (Some(minimum), Some(maximum)) = (setting.minimum, setting.maximum)
            && minimum > maximum
        {
            return Err(format!(
                "setting '{}' has minimum greater than maximum",
                setting.key
            ));
        }
        if setting
            .step
            .is_some_and(|step| !step.is_finite() || step <= 0.0)
        {
            return Err(format!("setting '{}' has an invalid step", setting.key));
        }
    }
    let mut group_ids = std::collections::HashSet::new();
    for group in &manifest.setting_groups {
        if !identifier(&group.id) || !group_ids.insert(group.id.as_str()) {
            return Err(format!(
                "setting group '{}' is invalid or duplicated",
                group.id
            ));
        }
    }
    for setting in &manifest.settings {
        if let Some(group) = &setting.group
            && !group_ids.contains(group.as_str())
        {
            return Err(format!(
                "setting '{}' references unknown group '{group}'",
                setting.key
            ));
        }
    }
    if !manifest.ui.sections.is_empty() && !manifest.ui.tabs.is_empty() {
        return Err("ui must use either sections or tabs, not both".into());
    }
    let validate_sections = |sections: &[UiSection]| -> Result<(), String> {
        for section in sections {
            for component in &section.components {
                if matches!(component.component_type, UiComponentType::Setting)
                    && component
                        .key
                        .as_deref()
                        .is_none_or(|key| !setting_keys.contains(key))
                {
                    return Err("ui setting component references an unknown setting".into());
                }
                if matches!(component.component_type, UiComponentType::Button)
                    && component
                        .action
                        .as_deref()
                        .is_none_or(|value| !identifier(value))
                {
                    return Err("ui button requires a valid action".into());
                }
                if matches!(component.component_type, UiComponentType::Link)
                    && component
                        .url
                        .as_deref()
                        .is_none_or(|url| !url.starts_with("https://"))
                {
                    return Err("ui links must use HTTPS".into());
                }
            }
        }
        Ok(())
    };
    validate_sections(&manifest.ui.sections)?;
    for tab in &manifest.ui.tabs {
        if !identifier(&tab.id) || tab.sections.is_empty() {
            return Err("ui tab id is invalid or contains no sections".into());
        }
        validate_sections(&tab.sections)?;
    }
    Ok(())
}

impl Deref for ModRegistry {
    type Target = HashMap<String, Mod>;

    fn deref(&self) -> &Self::Target {
        &self.mods
    }
}
