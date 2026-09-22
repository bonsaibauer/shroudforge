use std::{collections::HashSet, fs, sync::Arc};

use crate::{
    ModEnvironmentErrorReport, ModRegistry,
    alias::{Path, PathBuf},
};

struct ModEnvironmentInner {
    game_dir: PathBuf,
    cache_dir: PathBuf,
    mods_dir: PathBuf,

    registry: ModRegistry,
    disabled_mods: HashSet<String>,
}

#[derive(Clone)]
pub struct ModEnvironment {
    inner: Arc<ModEnvironmentInner>,
}

impl ModEnvironment {
    pub fn load(game_dir: impl AsRef<Path>) -> Result<Self, ModEnvironmentErrorReport> {
        let game_dir = game_dir.as_ref().to_path_buf();
        let cache_dir = game_dir.join(".cache");
        let mods_dir = game_dir.join("mods");

        let registry = ModRegistry::load(&mods_dir)?;
        let disabled_mods = read_disabled_mods(&game_dir);

        Ok(Self {
            inner: Arc::new(ModEnvironmentInner {
                game_dir,
                cache_dir,
                mods_dir,
                registry,
                disabled_mods,
            }),
        })
    }

    pub fn game_dir(&self) -> &Path {
        &self.inner.game_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.inner.cache_dir
    }

    pub fn mods_dir(&self) -> &Path {
        &self.inner.mods_dir
    }

    pub fn mod_registry(&self) -> &ModRegistry {
        &self.inner.registry
    }

    pub fn is_mod_enabled(&self, id: &str) -> bool {
        !self.inner.disabled_mods.contains(id)
    }

    pub fn enabled_mods(&self) -> impl Iterator<Item = &crate::Mod> {
        self.inner
            .registry
            .values()
            .filter(|r#mod| self.is_mod_enabled(&r#mod.info().id))
    }
}

fn read_disabled_mods(game_dir: &Path) -> HashSet<String> {
    fs::read(game_dir.join("config").join("shroudforge.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|config| {
            config
                .get("mods")
                .and_then(|mods| mods.as_object())
                .cloned()
        })
        .map(|mods| {
            mods.into_iter()
                .filter_map(|(id, config)| {
                    (config.get("enabled").and_then(|value| value.as_bool()) == Some(false))
                        .then_some(id)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn disabled_mods_remain_installed_but_are_not_enabled() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("shroudforge-enabled-mods-{suffix}"));
        let package = root.join("mods").join("example");
        fs::create_dir_all(&package).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(
            package.join("mod.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "mod.example",
                "name": "Example",
                "version": "1.0.0",
                "api": "^1.0.0",
                "capabilities": ["runtime"],
                "dependencies": [],
                "target": "client"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("config").join("shroudforge.json"),
            br#"{"mods":{"mod.example":{"enabled":false,"settings":{}}}}"#,
        )
        .unwrap();

        let environment = ModEnvironment::load(root.to_str().unwrap()).unwrap();
        assert!(environment.mod_registry().contains_key("mod.example"));
        assert!(!environment.is_mod_enabled("mod.example"));
        assert_eq!(environment.enabled_mods().count(), 0);

        fs::remove_dir_all(root).unwrap();
    }
}
