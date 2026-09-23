#[cfg(test)]
use std::fs;
use std::{collections::HashSet, sync::Arc};

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
        let registry = match ModRegistry::load(&mods_dir) {
            Ok(registry) => registry,
            Err(report) if report.error.is_none() => {
                for error in &report.mods {
                    tracing::error!(path = %error.path, error = %error.error, "Mod rejected");
                }
                report.mod_registry
            }
            Err(report) => return Err(report),
        };
        let disabled_mods = registry.values().filter(|item| !item.info().enabled).map(|item| item.info().id.clone()).collect();

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
        self.inner.registry.contains_key(id) && !self.inner.disabled_mods.contains(id)
    }

    pub fn enabled_mods(&self) -> impl Iterator<Item = &crate::Mod> {
        self.inner
            .registry
            .values()
            .filter(|r#mod| self.is_mod_enabled(&r#mod.info().id))
    }

    /// Shared dependency and target plan for asset execution and live mods.
    pub fn plan(&self, is_server: bool, api_version: &str) -> Vec<&crate::Mod> {
        self.plan_report(is_server, api_version).0
    }

    pub fn plan_report(&self, is_server: bool, api_version: &str) -> (Vec<&crate::Mod>, Vec<String>) {
        let blocked = match crate::compatibility::conflicts(self) {
            Ok(blocked) => blocked,
            Err(error) => return (Vec::new(), vec![format!("compatibility rules: {error}")]),
        };
        fn visit<'a>(
            env: &'a ModEnvironment,
            blocked: &std::collections::HashMap<String,String>,
            id: &str,
            server: bool,
            version: &semver::Version,
            visiting: &mut HashSet<String>,
            done: &mut HashSet<String>,
            order: &mut Vec<&'a crate::Mod>,
        ) -> Result<(), String> {
            if let Some(reason) = blocked.get(id) { return Err(format!("{id}: {reason}")); }
            if done.contains(id) {
                return Ok(());
            }
            if !visiting.insert(id.into()) {
                return Err(format!("dependency cycle: {id}"));
            }
            let item = env
                .mod_registry()
                .get(id)
                .ok_or_else(|| format!("missing dependency: {id}"))?;
            if !env.is_mod_enabled(id) {
                return Err(format!("disabled dependency: {id}"));
            }
            if matches!(
                (item.info().target, server),
                (crate::ModTarget::Client, true) | (crate::ModTarget::Server, false)
            ) {
                return Err(format!("{id}: wrong process target"));
            }
            if let Some(required) = &item.info().api {
                let required = semver::VersionReq::parse(required).map_err(|e| e.to_string())?;
                if !required.matches(version) {
                    return Err(format!("{id}: API {required} required"));
                }
            }
            for dependency in &item.info().dependencies {
                if dependency.id == "shroudforge-api" {
                    if !dependency.version.matches(version) {
                        return Err(format!("{id}: incompatible API dependency"));
                    }
                    continue;
                }
                let candidate = env.mod_registry().get(&dependency.id);
                if dependency.optional.unwrap_or(false)
                    && (candidate.is_none() || !env.is_mod_enabled(&dependency.id))
                {
                    continue;
                }
                let candidate =
                    candidate.ok_or_else(|| format!("{id}: missing {}", dependency.id))?;
                if !dependency.version.matches(&candidate.info().version) {
                    return Err(format!("{id}: incompatible {}", dependency.id));
                }
                visit(env, blocked, &dependency.id, server, version, visiting, done, order)?;
            }
            visiting.remove(id);
            done.insert(id.into());
            order.push(item);
            Ok(())
        }
        let version = semver::Version::parse(api_version).expect("API version is semver");
        let mut ids: Vec<_> = self
            .enabled_mods()
            .map(|item| item.info().id.clone())
            .collect();
        ids.sort();
        let mut done = HashSet::new();
        let mut order = Vec::new();
        let mut errors = Vec::new();
        for id in ids {
            if let Err(error) = visit(
                self,
                &blocked,
                &id,
                is_server,
                &version,
                &mut HashSet::new(),
                &mut done,
                &mut order,
            ) {
                tracing::error!(mod_id = %id, %error, "Mod excluded from execution plan");
                errors.push(error);
            }
        }
        (order, errors)
    }
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
            br#"{"schemaVersion":1,"logging":{"enabled":true,"minimumLevel":"INFO"},"modules":{},"mods":{"mod.example":{"enabled":false,"settings":{}}}}"#,
        )
        .unwrap();

        let migration_errors = crate::migration::migrate_installation(&root).unwrap();
        assert!(migration_errors.is_empty(), "{migration_errors:?}");

        let environment = ModEnvironment::load(root.to_str().unwrap()).unwrap();
        assert!(environment.mod_registry().contains_key("mod.example"));
        assert!(!environment.is_mod_enabled("mod.example"));
        assert_eq!(environment.enabled_mods().count(), 0);

        fs::remove_dir_all(root).unwrap();
    }
}
