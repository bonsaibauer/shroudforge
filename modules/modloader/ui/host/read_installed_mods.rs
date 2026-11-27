use super::read_mod_icons::read_mod_assets;
use super::ui_snapshot_types::ModDependencyInfo;
use super::ui_snapshot_types::ModInfo;
use shroudforge_package::config_revision;
use shroudforge_package::package_lookup::mod_ecosystem_for_package;
use std::fs;
use std::path::Path;

pub(crate) fn read_mods(root: &Path, locale: &str) -> Vec<ModInfo> {
    let Ok(entries) = shroudforge_package::modpack::package_locations(&shroudforge_loader_config::paths::mods_dir(root)) else {
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
        .into_iter()
        .filter_map(|location| {
            let package = &location.path;
            if !package.join("mod.json").is_file() && location.pack_id.is_some() {
                let id = location.id.clone();
                return Some((location, serde_json::json!({"id":id,"name":id,"version":"native","enabled":false,"settingValues":{},"settings":[],"groups":[]})));
            }
            shroudforge_package::config::read_manifest_path(root, &package)
                .map(|manifest| shroudforge_package::localized_manifest(&manifest, locale))
                .and_then(|manifest| {
                    serde_json::to_value(manifest)
                        .map(|value| (location, value))
                        .map_err(|error| error.to_string())
                })
                .ok()
        })
        .collect();
    let mut installed: std::collections::HashMap<String, (String, bool)> = std::collections::HashMap::new();
    for (location, value) in &manifests {
        if !location.pack_ready || !location.path.join("mod.json").is_file() { continue }
        let (Some(id), Some(version)) = (value["id"].as_str(), value["version"].as_str()) else { continue };
        let enabled = value["enabled"].as_bool().unwrap_or(false);
        if enabled || !installed.contains_key(id) {
            installed.insert(id.into(), (version.into(), enabled));
        }
    }
    let mut mods = Vec::new();
    for (location, value) in manifests {
        let package = &location.path;
        let native_only = !package.join("mod.json").is_file();
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
        let ecosystem = if native_only { "SF" } else { mod_ecosystem_for_package(&package) };
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
            instance_key: location.key(),
            pack_id: location.pack_id.clone(),
            native_only,
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
            environment: value
                .get("environment")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
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
            source: if location.pack_id.is_some() {
                "modpack"
            } else if catalog_mods.contains(id) {
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
            presets: value
                .get("presets")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
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
