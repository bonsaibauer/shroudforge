use crate::API_VERSION;
use mod_loader::ModEnvironment;
use std::path::Path;

pub(super) fn plan_runtime_mods<'a>(
    env: &'a ModEnvironment,
    root: &Path,
    is_server: bool,
    loader_config: &serde_json::Value,
) -> (
    Vec<&'a mod_loader::Mod>,
    serde_json::Map<String, serde_json::Value>,
) {
    tracing::debug!(target: "shroudforge::runtime", stage = "runtime-plan", "Runtime startup stage started");
    let (mut plan, plan_errors) = env.runtime_plan_report_detailed(is_server, API_VERSION);
    tracing::debug!(target: "shroudforge::runtime", stage = "runtime-plan", planned_mods = plan.len(), "Runtime startup stage completed");
    let mut errors = serde_json::Map::new();
    for issue in plan_errors {
        tracing::warn!(target: "shroudforge::runtime", mod_id = %issue.mod_id,
        code = %issue.code, detail = %issue.detail, "Mod is blocked by its execution plan");
        errors.insert(
            issue.mod_id,
            format!("plan-blocked: {}", issue.detail).into(),
        );
    }
    let world_editor_enabled = loader_config
        .pointer("/modules/worldEditor/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    if !world_editor_enabled {
        let mut disabled_mods = Vec::new();
        plan.retain(|item| {
            if item.info().id == "world-editor" {
                disabled_mods.push(item.info().id.clone());
                false
            } else {
                true
            }
        });
        for id in disabled_mods {
            errors.insert(
                id.clone(),
                "module-disabled: World Editor module is disabled in Settings. The World Editor mod is unavailable until it is enabled.".into(),
            );
            tracing::warn!(target: "shroudforge::runtime", mod_id = %id,
                "World Editor module is disabled in Settings. The World Editor mod will not be loaded");
        }
    }
    // Candidate plans contain disabled mods so the UI can toggle them live.
    // Enabled mods still require their non-optional dependencies to be
    // enabled and available before their startup callbacks may run.
    for item in &plan {
        if !item.info().enabled || errors.contains_key(&item.info().id) {
            continue;
        }
        let blocked_dependency = item.info().dependencies.iter().find_map(|dependency| {
            if dependency.id == "shroudforge-api"
                || dependency.id == mod_loader::SHROUDFORGE_RELEASE_DEPENDENCY
                || dependency.optional.unwrap_or(false)
            {
                return None;
            }
            let candidate = env.mod_registry().get(&dependency.id);
            if candidate.is_none_or(|candidate| !candidate.info().enabled) {
                Some(format!(
                    "required dependency is disabled or unavailable: {}",
                    dependency.id
                ))
            } else {
                errors
                    .get(&dependency.id)
                    .map(|_| format!("required dependency is blocked: {}", dependency.id))
            }
        });
        if let Some(detail) = blocked_dependency {
            tracing::warn!(target: "shroudforge::runtime", mod_id = %item.info().id,
            detail = %detail, "Runtime mod activation skipped");
            errors.insert(
                item.info().id.clone(),
                format!("dependency-blocked: {detail}").into(),
            );
        }
    }
    let needs_assets = plan.iter().any(|item| {
        item.info().enabled
            && item
                .info()
                .capabilities
                .contains(&mod_loader::Capability::Patch)
    });
    if needs_assets {
        let ready = mod_loader::prepared::fingerprint(env, is_server, API_VERSION)
            .map(|fingerprint| mod_loader::prepared::matches(&root, &fingerprint))
            .unwrap_or(false);
        if !ready {
            let mut blocked: std::collections::HashSet<String> = plan
                .iter()
                .filter(|item| {
                    item.info().enabled
                        && item
                            .info()
                            .capabilities
                            .contains(&mod_loader::Capability::Patch)
                })
                .map(|item| item.info().id.clone())
                .collect();
            loop {
                let before = blocked.len();
                for item in &plan {
                    if item.info().dependencies.iter().any(|dependency| {
                        !dependency.optional.unwrap_or(false) && blocked.contains(&dependency.id)
                    }) {
                        blocked.insert(item.info().id.clone());
                    }
                }
                if before == blocked.len() {
                    break;
                }
            }
            plan.retain(|item| {
            if blocked.contains(&item.info().id) { errors.insert(item.info().id.clone(), "startup-asset-apply-required: the early startup transaction did not publish this asset configuration".into()); false } else {true}
        });
        }
    }

    (plan, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::SystemTime};

    #[test]
    fn disabled_world_editor_module_does_not_enter_the_runtime_plan() {
        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("shroudforge-world-editor-module-{suffix}"));
        fs::create_dir_all(shroudforge_loader_config::paths::config_dir(&root)).unwrap();

        for id in ["world-editor", "independent-mod"] {
            let package = shroudforge_loader_config::paths::mods_dir(&root).join(id);
            fs::create_dir_all(package.join("src")).unwrap();
            fs::write(
                package.join("mod.json"),
                serde_json::to_vec(&serde_json::json!({
                    "id": id,
                    "name": id,
                    "version": "1.0.0",
                    "dependencies": [],
                    "capabilities": ["runtime"]
                }))
                .unwrap(),
            )
            .unwrap();
            fs::write(
                package.join("extended.mod.json"),
                br#"{"schemaVersion":1,"enabled":true,"targets":["client","server"]}"#,
            )
            .unwrap();
            fs::write(package.join("src/mod.lua"), "return {}\n").unwrap();
        }
        fs::write(
            shroudforge_loader_config::paths::loader_config(&root),
            br#"{"schemaVersion":1,"modules":{"worldEditor":{"enabled":false}}}"#,
        )
        .unwrap();

        let environment = ModEnvironment::load(root.to_str().unwrap()).unwrap();
        let config = shroudforge_loader_config::config::read_loader(&root).unwrap();
        let (plan, errors) = plan_runtime_mods(&environment, &root, false, &config);

        assert!(plan.iter().all(|item| item.info().id != "world-editor"));
        assert!(plan.iter().any(|item| item.info().id == "independent-mod"));
        assert!(errors["world-editor"]
            .as_str()
            .unwrap()
            .contains("disabled in Settings"));

        fs::remove_dir_all(root).unwrap();
    }
}
