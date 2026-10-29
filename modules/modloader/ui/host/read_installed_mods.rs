use super::read_mod_icons::read_mod_assets;
use super::ui_snapshot_types::ModDependencyInfo;
use super::ui_snapshot_types::ModInfo;
use shroudforge_package::config_revision;
use shroudforge_package::package_lookup::mod_ecosystem_for_package;
use std::fs;
use std::path::Path;

pub(crate) fn read_mods(root: &Path, locale: &str) -> Vec<ModInfo> {
    let Ok(entries) = fs::read_dir(shroudforge_loader_config::paths::mods_dir(root)) else {
        return Vec::new();
    };
    let catalog_mods: std::collections::HashSet<String> =
        fs::read(shroudforge_loader_config::paths::state_file(root))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|state| state.get("catalog").cloned())
            .and_then(|catalog| catalog.get("projects").cloned())
            .and_then(|projects| projects.as_object().cloned())
            .into_iter()
            .flat_map(|projects| projects.into_values())
            .filter_map(|project| project.get("modId")?.as_str().map(str::to_owned))
            .collect();
    let manifests: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let package = entry.path();
            shroudforge_package::config::read_manifest_path(root, &package)
                .map(|manifest| shroudforge_package::localized_manifest(&manifest, locale))
                .and_then(|manifest| {
                    serde_json::to_value(manifest)
                        .map(|value| (package, value))
                        .map_err(|error| error.to_string())
                })
                .ok()
        })
        .collect();
    let installed: std::collections::HashMap<String, (String, bool)> = manifests
        .iter()
        .filter_map(|(_, value)| {
            Some((
                value.get("id")?.as_str()?.to_owned(),
                (
                    value.get("version")?.as_str()?.to_owned(),
                    value
                        .get("enabled")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                ),
            ))
        })
        .collect();
    let mut mods = Vec::new();
    for (package, value) in manifests {
        let Some(id) = value.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let config =
            serde_json::json!({"enabled":value["enabled"],"settings":value["settingValues"]});
        let capabilities = value
            .get("capabilities")
            .and_then(serde_json::Value::as_array);
        let asset = capabilities.is_some_and(|items| items.iter().any(|item| item == "patch"));
        let runtime = capabilities.is_some_and(|items| {
            items
                .iter()
                .any(|item| item == "export" || item == "runtime" || item == "runtime-register-dll")
        });
        let revision = config_revision(&config);
        let ecosystem = mod_ecosystem_for_package(&package);
        let icon = value
            .get("icon")
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        let assets = read_mod_assets(&package, icon.as_deref());
        let dependencies = value
            .get("dependencies")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|dependency| {
                let id = dependency.get("id")?.as_str()?.to_owned();
                let version = dependency.get("version")?.as_str()?.to_owned();
                let optional = dependency
                    .get("optional")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let installed_version = installed.get(&id).map(|(version, _)| version.clone());
                let status = if id == "shroudforge-api" {
                    "loader-api"
                } else if let Some((candidate_version, enabled)) = installed.get(&id) {
                    match semver::VersionReq::parse(&version)
                        .ok()
                        .zip(semver::Version::parse(candidate_version).ok())
                    {
                        Some((requirement, candidate)) if !requirement.matches(&candidate) => {
                            "version-mismatch"
                        }
                        Some(_) if !*enabled && optional => "optional-disabled",
                        Some(_) if !*enabled => "disabled",
                        Some(_) => "installed",
                        None => "invalid-version",
                    }
                } else if optional {
                    "optional-missing"
                } else {
                    "missing"
                };
                Some(ModDependencyInfo {
                    id,
                    version,
                    optional,
                    installed_version,
                    status: status.into(),
                })
            })
            .collect();
        mods.push(ModInfo {
            revision,
            id: id.into(),
            name: value
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or(id)
                .into(),
            version: value
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .into(),
            dependencies,
            ecosystem,
            target: value
                .get("target")
                .and_then(|value| value.as_str())
                .unwrap_or("client, server")
                .into(),
            asset,
            runtime,
            description: value
                .get("description")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
            authors: value
                .get("authors")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            license: value
                .get("license")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
            icon,
            links: serde_json::from_value(value.get("links").cloned().unwrap_or_default())
                .unwrap_or_default(),
            source: if catalog_mods.contains(id) {
                "shroudedit"
            } else {
                "local"
            },
            enabled: config
                .get("enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            settings: value
                .get("settings")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default(),
            groups: value
                .get("groups")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default(),
            changelog: value
                .get("changelog")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .take(50)
                        .collect()
                })
                .unwrap_or_default(),
            assets,
            setting_values: config
                .get("settings")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
            input_contexts: value
                .get("inputContexts")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        });
    }
    mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    mods
}
