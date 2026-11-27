use super::{api_cache, ModUpdateCheckLock};
use crate::{
    catalog_item::{CatalogItem, CatalogSearchResult},
    mod_package::ResolveContentPlan,
    provider_config::CatalogProviderConfig,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

const CATALOG_SEARCH_CACHE_SECONDS: u64 = 15 * 60;
const CATALOG_DETAIL_CACHE_SECONDS: u64 = 60 * 60;
const CATALOG_OPTIONS_CACHE_SECONDS: u64 = 24 * 60 * 60;
const CATALOG_UPDATE_CACHE_SECONDS: u64 = 6 * 60 * 60;
const CATALOG_PROJECT_LOOKUP_CACHE_SECONDS: u64 = 24 * 60 * 60;

pub fn clear_catalog_api_cache(root: &Path) -> Result<(), String> {
    api_cache::clear(root)
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct CatalogApiOptions {
    categories_by_project_type: HashMap<String, Vec<String>>,
    game_versions: Vec<String>,
    project_types: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct CachedCatalogVersions {
    complete: bool,
    versions: Vec<serde_json::Value>,
}

pub(crate) enum CatalogVersionStop<'a> {
    Id(&'a str),
    Number(&'a str),
}

impl CatalogVersionStop<'_> {
    fn cache_key(&self) -> serde_json::Value {
        match self {
            Self::Id(value) => serde_json::json!(["id", value]),
            Self::Number(value) => serde_json::json!(["number", value]),
        }
    }
}

pub fn resolve_catalog_project(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    existing_project_ids: &[String],
) -> Result<ResolveContentPlan, String> {
    resolve_catalog_version(
        root,
        client,
        provider,
        project_id,
        existing_project_ids,
        None,
    )
}

pub fn resolve_catalog_version(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    existing_project_ids: &[String],
    version_id: Option<&str>,
) -> Result<ResolveContentPlan, String> {
    resolve_catalog_content_version(
        root,
        client,
        provider,
        project_id,
        existing_project_ids,
        version_id,
        "mod",
    )
}

pub fn resolve_catalog_content_version(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    existing_project_ids: &[String],
    version_id: Option<&str>,
    content_type: &str,
) -> Result<ResolveContentPlan, String> {
    if !matches!(content_type, "mod" | "modpack") {
        return Err("unsupported resolvable ShroudEdit content type".into());
    }
    let loaders = if content_type == "modpack" {
        vec!["sfpack"]
    } else if provider.loader.eq_ignore_ascii_case("shroudforge") {
        vec!["shroudforge", "eml"]
    } else {
        vec![provider.loader.as_str()]
    };
    let game_versions = current_game_version(root).into_iter().collect::<Vec<_>>();
    let response = client
        .post(format!(
            "{}/v3/content/resolve",
            provider.base_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "project_id": project_id,
            "version_id": version_id,
            "content_type": content_type,
            "selected": { "game_versions": game_versions.clone(), "loaders": loaders.clone() },
            "target": { "game_versions": game_versions, "loaders": loaders },
            "existing_project_ids": existing_project_ids
        }))
        .send()
        .map_err(|error| format!("ShroudEdit is unreachable: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "ShroudEdit resolution returned HTTP {}",
            response.status()
        ));
    }
    response
        .json()
        .map_err(|error| format!("Invalid installation plan: {error}"))
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledCatalogProject {
    mod_id: String,
    version_id: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackMember {
    pub project_id: String,
    pub version_id: String,
    pub mod_id: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCatalogPack {
    #[serde(default)]
    pub pack_id: String,
    pub version_id: String,
    pub version_number: String,
    #[serde(default)]
    pub selected_optional_ids: Vec<String>,
    pub mods: Vec<InstalledPackMember>,
}

pub fn installed_catalog_packs(root: &Path) -> Vec<(String, InstalledCatalogPack)> {
    read_catalog_install_registry(root)["packs"]
        .as_object()
        .into_iter()
        .flat_map(|packs| packs.iter())
        .filter_map(|(id, value)| {
            serde_json::from_value(value.clone())
                .ok()
                .map(|pack| (id.clone(), pack))
        })
        .collect()
}

struct InstalledModPackage {
    id: String,
    name: String,
    version: String,
    release_channel: Option<shroudforge_package::ModReleaseChannel>,
    package: PathBuf,
}

struct CatalogModVersion {
    id: String,
    project_id: String,
    version_number: String,
    installed_version_id: Option<String>,
}

fn installed_mod_packages(root: &Path) -> Vec<InstalledModPackage> {
    shroudforge_package::modpack::package_locations(&shroudforge_loader_config::paths::mods_dir(
        root,
    ))
    .unwrap_or_default()
    .into_iter()
    .filter(|entry| entry.pack_id.is_none())
    .filter_map(|entry| {
        let package = entry.path;
        if !package.is_dir() {
            return None;
        }
        let manifest = shroudforge_package::config::read_manifest_path(root, &package).ok()?;
        Some(InstalledModPackage {
            id: manifest.id,
            name: manifest.name,
            version: manifest.version.to_string(),
            release_channel: manifest.release_channel,
            package,
        })
    })
    .collect()
}

fn lookup_catalog_mod_by_manifest_id(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    mod_id: &str,
    game_version: Option<&str>,
    installed_version: &str,
    release_channel: Option<shroudforge_package::ModReleaseChannel>,
) -> Result<Option<CatalogModVersion>, String> {
    let base = provider.base_url.trim_end_matches('/');
    let project_key = api_cache::key("project-lookup", base, &serde_json::json!(mod_id));
    let project = if let Some(cached) = api_cache::read::<serde_json::Value>(
        root,
        &project_key,
        CATALOG_PROJECT_LOOKUP_CACHE_SECONDS,
    ) {
        if cached.is_null() {
            return Ok(None);
        }
        cached
    } else {
        let response = client
            .get(format!("{base}/v3/project/{mod_id}"))
            .send()
            .map_err(|error| format!("ShroudEdit is unreachable: {error}"))?;
        if response.status().as_u16() == 404 {
            api_cache::write(root, project_key, &serde_json::Value::Null);
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!(
                "ShroudEdit project lookup returned HTTP {}",
                response.status()
            ));
        }
        let project: serde_json::Value = response
            .json()
            .map_err(|error| format!("Invalid ShroudEdit project response: {error}"))?;
        project
    };
    let project_id = string_field(&project, &["id", "project_id"]);
    if project_id.is_empty() || project["slug"].as_str() != Some(mod_id) {
        api_cache::write(
            root,
            api_cache::key("project-lookup", base, &serde_json::json!(mod_id)),
            &serde_json::Value::Null,
        );
        return Ok(None);
    }
    api_cache::write(
        root,
        project_key,
        &serde_json::json!({"id": project_id, "slug": mod_id}),
    );
    let loaders =
        serde_json::to_string(&["shroudforge", "eml"]).map_err(|error| error.to_string())?;
    let versions = fetch_cached_catalog_mod_versions(
        root,
        client,
        base,
        &project_id,
        &loaders,
        CatalogVersionStop::Number(installed_version),
    )?;
    let installed_version_id = versions
        .iter()
        .find(|version| version["version_number"].as_str() == Some(installed_version))
        .and_then(|version| version["id"].as_str())
        .map(str::to_owned);
    let latest = versions
        .iter()
        .find(|version| {
            let candidate_channel = version["version_type"].as_str().unwrap_or("release");
            let allowed = match release_channel {
                Some(shroudforge_package::ModReleaseChannel::Alpha) => {
                    matches!(candidate_channel, "alpha" | "beta" | "release")
                }
                Some(shroudforge_package::ModReleaseChannel::Beta) => {
                    matches!(candidate_channel, "beta" | "release")
                }
                _ => candidate_channel == "release",
            };
            allowed
                && game_version.is_none_or(|game| {
                    version["game_versions"]
                        .as_array()
                        .is_some_and(|games| games.iter().any(|value| value.as_str() == Some(game)))
                })
        })
        .or_else(|| {
            versions
                .iter()
                .find(|version| version["version_number"].as_str() == Some(installed_version))
        });
    Ok(latest.and_then(|version| {
        Some(CatalogModVersion {
            id: string_field(version, &["id"]),
            project_id,
            version_number: string_field(version, &["version_number"]),
            installed_version_id,
        })
    }))
}

pub(crate) fn fetch_catalog_mod_versions(
    client: &reqwest::blocking::Client,
    base: &str,
    project_id: &str,
    loaders: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut versions = Vec::new();
    for offset in (0..10_000).step_by(100) {
        let page = fetch_catalog_mod_version_page(client, base, project_id, loaders, offset)?;
        let count = page.len();
        versions.extend(page);
        if count < 100 {
            return Ok(versions);
        }
    }
    Err("ShroudEdit project has more than 10,000 versions".into())
}

fn fetch_catalog_mod_version_page(
    client: &reqwest::blocking::Client,
    base: &str,
    project_id: &str,
    loaders: &str,
    offset: usize,
) -> Result<Vec<serde_json::Value>, String> {
    let offset = offset.to_string();
    let response = client
        .get(format!("{base}/v3/project/{project_id}/version"))
        .query(&[
            ("loaders", loaders),
            ("limit", "100"),
            ("offset", offset.as_str()),
            ("include_changelog", "false"),
        ])
        .send()
        .map_err(|error| format!("Could not read ShroudEdit versions: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "ShroudEdit version lookup returned HTTP {}",
            response.status()
        ));
    }
    response
        .json::<Vec<serde_json::Value>>()
        .map_err(|error| format!("Invalid ShroudEdit versions response: {error}"))
}

pub(crate) fn fetch_cached_catalog_mod_versions(
    root: &Path,
    client: &reqwest::blocking::Client,
    base: &str,
    project_id: &str,
    loaders: &str,
    stop_at: CatalogVersionStop<'_>,
) -> Result<Vec<serde_json::Value>, String> {
    let cache_key = api_cache::key(
        "project-versions",
        base,
        &serde_json::json!([project_id, loaders, stop_at.cache_key()]),
    );
    if let Some(cached) = api_cache::read::<CachedCatalogVersions>(
        root,
        &cache_key,
        CATALOG_UPDATE_CACHE_SECONDS,
    ) {
        if cached.complete
            || cached
                .versions
                .iter()
                .any(|version| catalog_version_matches(version, &stop_at))
        {
            return Ok(cached.versions);
        }
    }

    let mut versions = Vec::new();
    let mut complete = false;
    for offset in (0..10_000).step_by(100) {
        let page = fetch_catalog_mod_version_page(client, base, project_id, loaders, offset)?;
        let count = page.len();
        versions.extend(page.into_iter().map(compact_catalog_version));
        if let Some(position) = versions
            .iter()
            .position(|version| catalog_version_matches(version, &stop_at))
        {
            versions.truncate(position + 1);
            break;
        }
        if count < 100 {
            complete = true;
            break;
        }
    }
    if !complete && versions.len() >= 10_000 {
        return Err("ShroudEdit project has more than 10,000 versions".into());
    }
    let cached = CachedCatalogVersions { complete, versions };
    api_cache::write(root, cache_key, &cached);
    Ok(cached.versions)
}

fn compact_catalog_version(version: serde_json::Value) -> serde_json::Value {
    let Some(fields) = version.as_object() else {
        return version;
    };
    let mut compact = serde_json::Map::new();
    for field in [
        "id",
        "project_id",
        "version_number",
        "date_published",
        "version_type",
        "game_versions",
        "project_types",
    ] {
        if let Some(value) = fields.get(field) {
            compact.insert(field.to_owned(), value.clone());
        }
    }
    serde_json::Value::Object(compact)
}

fn catalog_version_matches(version: &serde_json::Value, stop_at: &CatalogVersionStop<'_>) -> bool {
    match stop_at {
        CatalogVersionStop::Id(id) => version["id"].as_str() == Some(*id),
        CatalogVersionStop::Number(number) => {
            version["version_number"].as_str() == Some(*number)
        }
    }
}

pub(crate) fn current_game_version(root: &Path) -> Option<String> {
    std::fs::read(shroudforge_loader_config::paths::version_file(root))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value["gameVersion"].as_str().map(str::to_owned))
        .filter(|version| !version.trim().is_empty() && version != "Automatisch erkannt")
}

fn newer_catalog_version_for_registered_mod(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    installed_version_id: &str,
    installed_version: &str,
    game_version: Option<&str>,
) -> Result<Option<CatalogModVersion>, String> {
    let base = provider.base_url.trim_end_matches('/');
    let loaders =
        serde_json::to_string(&["shroudforge", "eml"]).map_err(|error| error.to_string())?;
    let mut versions = fetch_cached_catalog_mod_versions(
        root,
        client,
        base,
        project_id,
        &loaders,
        CatalogVersionStop::Id(installed_version_id),
    )?;
    versions.sort_by(|left, right| {
        right["date_published"]
            .as_str()
            .unwrap_or_default()
            .cmp(left["date_published"].as_str().unwrap_or_default())
    });
    let position = versions.iter().position(|version| version["id"].as_str() == Some(installed_version_id))
        .ok_or_else(|| format!("Installed ShroudEdit version {installed_version_id} is no longer listed. Select an update manually."))?;
    let current = &versions[position];
    if current["version_number"].as_str() != Some(installed_version) {
        return Err(format!(
            "Installed mod version differs from ShroudEdit version {installed_version_id}. Select an update manually."
        ));
    }
    let channel = current["version_type"].as_str().unwrap_or("release");
    let installed_semver = Version::parse(installed_version).ok();
    Ok(versions[..position].iter().find_map(|version| {
        let candidate_channel = version["version_type"].as_str().unwrap_or("release");
        let allowed = match channel {
            "alpha" => matches!(candidate_channel, "alpha" | "beta" | "release"),
            "beta" => matches!(candidate_channel, "beta" | "release"),
            _ => candidate_channel == "release",
        };
        if !allowed
            || game_version.is_some_and(|game| {
                !version["game_versions"]
                    .as_array()
                    .is_some_and(|games| games.iter().any(|value| value.as_str() == Some(game)))
            })
        {
            return None;
        }
        let number = version["version_number"].as_str()?;
        let candidate_semver = Version::parse(number).ok();
        if !installed_semver
            .as_ref()
            .zip(candidate_semver.as_ref())
            .is_some_and(|(current, candidate)| candidate >= current)
        {
            return None;
        }
        Some(CatalogModVersion {
            id: version["id"].as_str()?.to_owned(),
            project_id: project_id.to_owned(),
            version_number: number.to_owned(),
            installed_version_id: None,
        })
    }))
}

pub fn ensure_catalog_project_for_installed_mod(
    root: &Path,
    provider: &CatalogProviderConfig,
    mod_id: &str,
) -> Result<Option<String>, String> {
    if let Some(project_id) = catalog_project_for_mod(root, mod_id) {
        return Ok(Some(project_id));
    }
    let Some(package) = shroudforge_package::package_lookup::find_mod_package(root, mod_id) else {
        return Ok(None);
    };
    let manifest = shroudforge_package::config::read_manifest_path(root, &package)
        .map_err(|error| format!("Could not read installed mod {mod_id}: {error}"))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(format!(
            "ShroudForge/{} (mod-project-lookup)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let Some(latest) = lookup_catalog_mod_by_manifest_id(
        root,
        &client,
        provider,
        mod_id,
        current_game_version(root).as_deref(),
        &manifest.version.to_string(),
        manifest.release_channel,
    )?
    else {
        return Ok(None);
    };
    let current_version_id = latest.installed_version_id.unwrap_or_default();
    remember_catalog_install(
        root,
        &latest.project_id,
        &current_version_id,
        mod_id,
        &package,
    )?;
    Ok(Some(latest.project_id))
}

pub fn check_catalog_mod_updates(root: &Path, manual: bool) -> Result<serde_json::Value, String> {
    let _check_lock = ModUpdateCheckLock::try_acquire(root)?
        .ok_or("A mod update check is already running for this installation")?;
    let config = shroudforge_loader_config::config::read_loader(root)?;
    if config
        .pointer("/modules/updates/enabled")
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        return Err(crate::UPDATE_MODULE_DISABLED_MESSAGE.into());
    }
    if !manual
        && config
            .pointer("/modules/updates/automatic/enabled")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
    {
        return Err("Automatic update checks are disabled".into());
    }
    let queue_updates = config
        .pointer("/modules/updates/automatic/queueUpdates")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let provider = config
        .pointer("/catalog/provider")
        .cloned()
        .map(serde_json::from_value::<CatalogProviderConfig>)
        .transpose()
        .map_err(|error| format!("invalid catalog provider: {error}"))?
        .unwrap_or_default();
    if provider.kind != "shroudedit" {
        return Err("unsupported catalog provider".into());
    }
    let registry = read_catalog_install_registry(root);
    let projects = registry
        .get("projects")
        .and_then(serde_json::Value::as_object)
        .ok_or("catalog install registry has no project list")?;
    let registered_projects = projects
        .iter()
        .filter_map(|(project_id, value)| {
            serde_json::from_value::<InstalledCatalogProject>(value.clone())
                .ok()
                .map(|installed| (installed.mod_id, (project_id.clone(), installed.version_id)))
        })
        .collect::<HashMap<_, _>>();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(format!(
            "ShroudForge/{} (mod-update-check)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let mut latest_updates = Vec::new();
    let mut failures = Vec::new();
    let mut checked = 0usize;
    let protected = 0usize;
    let game_version = current_game_version(root);
    for installed in installed_mod_packages(root) {
        let result = if let Some((project_id, installed_version_id)) =
            registered_projects.get(&installed.id)
        {
            if installed_version_id.is_empty() {
                lookup_catalog_mod_by_manifest_id(
                    root,
                    &client,
                    &provider,
                    &installed.id,
                    game_version.as_deref(),
                    &installed.version,
                    installed.release_channel,
                )
                .and_then(|latest| {
                    let Some(latest) = latest else {
                        return Ok(None);
                    };
                    if latest.project_id != *project_id {
                        return Err(format!(
                            "Installed mod '{}' is linked to a different ShroudEdit project",
                            installed.id
                        ));
                    }
                    Ok(Some((
                        project_id.clone(),
                        latest.id,
                        latest.installed_version_id.unwrap_or_default(),
                        Some(latest.version_number),
                    )))
                })
            } else {
                newer_catalog_version_for_registered_mod(
                    root,
                    &client,
                    &provider,
                    project_id,
                    installed_version_id,
                    &installed.version,
                    game_version.as_deref(),
                )
                .map(|candidate| {
                    candidate.map(|candidate| {
                        (
                            project_id.clone(),
                            candidate.id,
                            installed_version_id.clone(),
                            Some(candidate.version_number),
                        )
                    })
                })
            }
        } else {
            match lookup_catalog_mod_by_manifest_id(
                root,
                &client,
                &provider,
                &installed.id,
                game_version.as_deref(),
                &installed.version,
                installed.release_channel,
            ) {
                Ok(Some(latest)) => {
                    let installed_version_id = latest.installed_version_id.unwrap_or_default();
                    if let Err(error) = remember_catalog_install(
                        root,
                        &latest.project_id,
                        &installed_version_id,
                        &installed.id,
                        &installed.package,
                    ) {
                        failures.push(format!("{}: {error}", installed.id));
                        continue;
                    }
                    Ok(Some((
                        latest.project_id,
                        latest.id,
                        installed_version_id,
                        Some(latest.version_number),
                    )))
                }
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            }
        };
        match result {
            Ok(Some((project_id, latest_version_id, installed_version_id, latest_number))) => {
                checked += 1;
                if latest_version_id != installed_version_id {
                    let latest_version = latest_number.or_else(|| {
                        client
                            .get(format!(
                                "{}/v3/version/{latest_version_id}",
                                provider.base_url.trim_end_matches('/')
                            ))
                            .send()
                            .ok()
                            .filter(|response| response.status().is_success())
                            .and_then(|response| {
                                response.json::<crate::mod_package::ApiVersion>().ok()
                            })
                            .filter(|version| {
                                version.id == latest_version_id && version.project_id == project_id
                            })
                            .map(|version| version._version_number)
                    });
                    let is_newer = latest_version.as_deref().is_some_and(|latest| {
                        catalog_candidate_is_newer(
                            latest,
                            &installed.version,
                            !installed_version_id.is_empty(),
                        )
                    });
                    if is_newer {
                        latest_updates.push(serde_json::json!({
                            "projectId": project_id,
                            "modId": installed.id,
                            "name": installed.name,
                            "currentVersion": installed.version,
                            "latestVersion": latest_version,
                            "latestVersionId": latest_version_id
                        }));
                    }
                }
            }
            Ok(None) => checked += 1,
            Err(error) => failures.push(format!("{}: {error}", installed.id)),
        }
    }
    let checked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut queued = 0usize;
    if queue_updates {
        let provider_value = serde_json::to_value(&provider).map_err(|error| error.to_string())?;
        for update in &latest_updates {
            let Some(project_id) = update["projectId"].as_str() else {
                continue;
            };
            let Some(mod_id) = update["modId"].as_str() else {
                continue;
            };
            let title = update["name"].as_str().unwrap_or(mod_id);
            match crate::request_mod_update_if_needed(
                root,
                project_id,
                mod_id,
                title,
                provider_value.clone(),
                update["latestVersionId"]
                    .as_str()
                    .ok_or("update has no version ID")?,
            ) {
                Ok(true) => queued += 1,
                Ok(false) => {}
                Err(error) => failures.push(format!("Could not queue {mod_id}: {error}")),
            }
        }
    }
    let result = serde_json::json!({
        "checked": checked,
        "protected": protected,
        "available": latest_updates.len(),
        "queued": queued,
        "failed": failures.len(),
        "checkedAt": checked_at,
        "latestUpdates": latest_updates.clone(),
        "errors": failures.clone()
    });
    shroudforge_loader_config::config::update_loader(root, |config| {
        config["modules"]["updates"]["mods"]["lastCheckAt"] = checked_at.into();
        config["modules"]["updates"]["mods"]["latestUpdates"] = result["latestUpdates"].clone();
        config["modules"]["updates"]["mods"]["lastCheckResult"] = serde_json::json!({
            "checked": checked,
            "available": latest_updates.len(),
            "failed": failures.len(),
            "checkedAt": checked_at,
            "errors": failures
        });
        Ok(())
    })?;
    shroudforge_logging::write_activity(
        root,
        "ShroudEdit",
        "Check mod updates",
        if failures.is_empty() {
            "Succeeded"
        } else {
            "Completed with errors"
        },
        Some(&format!(
            "Checked {checked} installed mod(s), found {} update(s), queued {queued}, {} failure(s).",
            latest_updates.len(),
            failures.len()
        )),
        if failures.is_empty() { "info" } else { "warn" },
    );
    Ok(result)
}

pub fn fetch_catalog(
    root: &Path,
    provider: &CatalogProviderConfig,
    search: CatalogSearchParameters,
) -> Result<CatalogSearchResult, String> {
    if provider.kind != "shroudedit" {
        return Err(format!("unsupported catalog provider: {}", provider.kind));
    }
    let query = search.query.trim().chars().take(100).collect::<String>();
    let project_type = match search.project_type.as_str() {
        "schematic" => "schematic",
        "modpack" => "modpack",
        "server" => "server",
        _ => "mod",
    }
    .to_owned();
    let loader = safe_filter_values(&search.loader)
        .into_iter()
        .filter(|value| matches!(value.as_str(), "shroudforge" | "eml"))
        .collect::<Vec<_>>();
    let category = safe_filter_values(&search.category);
    let game_version = safe_filter_values(&search.game_version);
    let sort = match search.sort.as_str() {
        "downloads" => "downloads".to_owned(),
        "newest" => "newest".to_owned(),
        "updated" => "updated".to_owned(),
        _ => "relevance".to_owned(),
    };
    let page = search.page.clamp(1, 10_000);
    let limit = match search.limit {
        40 | 60 => search.limit,
        _ => 20,
    };
    let offset = ((page - 1) * limit).to_string();
    let limit_parameter = limit.to_string();
    let facets = catalog_search_facets_for_type(
        &project_type,
        &loader,
        &category,
        &game_version,
        &search.environment,
    );
    let facets = serde_json::to_string(&facets).map_err(|error| error.to_string())?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(format!(
            "ShroudForge/{} (catalog)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let search_key = api_cache::key(
        "search-v2",
        &provider.base_url,
        &serde_json::json!({
            "query": query,
            "projectType": project_type,
            "loader": loader,
            "category": category,
            "gameVersion": game_version,
            "environment": search.environment,
            "sort": sort,
            "page": page,
            "limit": limit
        }),
    );
    if let Some(mut cached) =
        api_cache::read::<CatalogSearchResult>(root, &search_key, CATALOG_SEARCH_CACHE_SECONDS)
    {
        let options = fetch_cached_catalog_api_options(root, &client, provider);
        cached.categories = options
            .categories_by_project_type
            .get(&cached.project_type)
            .cloned()
            .unwrap_or_default();
        cached.game_versions = options.game_versions;
        cached.project_types = options.project_types;
        for item in &mut cached.items {
            item.installed_mod_id = (item.project_type == "mod")
                .then(|| installed_mod_for_catalog_project(root, &item.id, &item.slug))
                .flatten();
        }
        return Ok(cached);
    }
    let response = client
        .get(format!(
            "{}/v3/search",
            provider.base_url.trim_end_matches('/')
        ))
        .query(&[
            ("query", query.as_str()),
            ("limit", limit_parameter.as_str()),
            ("offset", offset.as_str()),
            ("index", sort.as_str()),
            ("facets", facets.as_str()),
        ])
        .send()
        .map_err(|error| format!("ShroudEdit is unreachable: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("ShroudEdit returned HTTP {}", response.status()));
    }
    let payload: serde_json::Value = response
        .json()
        .map_err(|error| format!("Invalid ShroudEdit response: {error}"))?;
    let values = payload
        .as_array()
        .or_else(|| payload.get("hits").and_then(|value| value.as_array()))
        .or_else(|| payload.get("projects").and_then(|value| value.as_array()))
        .or_else(|| payload.get("data").and_then(|value| value.as_array()))
        .ok_or("ShroudEdit search response contains no result list")?;
    let project_ids = values
        .iter()
        .map(|value| string_field(value, &["project_id", "projectId", "id"]))
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    let projects = fetch_catalog_projects(
        &client,
        provider.base_url.trim_end_matches('/'),
        &project_ids,
    )?;
    let version_ids = values
        .iter()
        .filter_map(|value| value.get("version_id").and_then(|value| value.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let version_summaries = fetch_version_summaries(&client, provider, &version_ids);
    let mut items = Vec::new();
    for value in values {
        let loaders = value
            .get("loaders")
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let id = string_field(value, &["project_id", "projectId", "id"]);
        let slug = string_field(value, &["slug", "project_slug"]);
        let slug = if slug.is_empty() {
            string_field(value, &["project_id", "projectId", "id"])
        } else {
            slug
        };
        let name = string_field(value, &["name", "title"]);
        if id.is_empty() || name.is_empty() {
            continue;
        }
        let version_id = optional_string_field(value, &["version_id", "latest_version_id"]);
        let version = version_id
            .as_ref()
            .and_then(|id| {
                version_summaries
                    .get(id)
                    .map(|summary| summary.number.clone())
            })
            .or_else(|| optional_string_field(value, &["latest_version", "version"]));
        let environment = version_id
            .as_ref()
            .and_then(|id| version_summaries.get(id))
            .and_then(|summary| summary.environment.clone())
            .or_else(|| {
                let environments = string_values(value.get("environment"));
                (environments.len() == 1).then(|| environments[0].clone())
            });
        let project_loader_fields = value.get("project_loader_fields");
        let game_versions = string_values(
            value
                .get("game_versions")
                .or_else(|| project_loader_fields.and_then(|fields| fields.get("game_versions"))),
        );
        let icon_url = projects.get(&id).and_then(catalog_icon_url);
        let installed_mod_id = installed_mod_for_catalog_project(root, &id, &slug);
        let project_types = string_values(value.get("project_types"));
        items.push(CatalogItem {
            id,
            slug: slug.clone(),
            project_type: project_type.clone(),
            project_types,
            name,
            summary: string_field(value, &["summary", "description"]),
            icon_url,
            author: optional_string_field(value, &["author", "owner_name", "ownerName"]),
            downloads: value
                .get("downloads")
                .or_else(|| value.get("download_count"))
                .and_then(|value| value.as_u64()),
            follows: value.get("follows").and_then(|value| value.as_u64()),
            version,
            environment,
            date_modified: optional_string_field(
                value,
                &["date_modified", "dateModified", "updated_at", "updatedAt"],
            ),
            latest_version_id: version_id,
            loaders,
            categories: string_values(
                value
                    .get("display_categories")
                    .or_else(|| value.get("categories")),
            ),
            game_versions,
            gallery: string_values(value.get("gallery")),
            installed_mod_id: (project_type == "mod")
                .then_some(installed_mod_id)
                .flatten(),
        });
    }
    let options = fetch_cached_catalog_api_options(root, &client, provider);
    let categories = options
        .categories_by_project_type
        .get(&project_type)
        .cloned()
        .unwrap_or_default();
    let result = CatalogSearchResult {
        query,
        project_type,
        project_types: options.project_types,
        loader,
        category,
        game_version,
        sort,
        page,
        limit,
        total: payload
            .get("total_hits")
            .or_else(|| payload.get("total"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(items.len() as u64) as usize,
        items,
        categories,
        game_versions: options.game_versions,
    };
    if !result.items.is_empty() {
        api_cache::write(root, search_key, &result);
    }
    Ok(result)
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CatalogSearchParameters {
    pub query: String,
    pub project_type: String,
    pub loader: Vec<String>,
    pub category: Vec<String>,
    pub game_version: Vec<String>,
    pub environment: Vec<String>,
    pub sort: String,
    pub page: usize,
    pub limit: usize,
}

fn safe_filter_value(value: &str) -> String {
    value
        .trim()
        .chars()
        .take(80)
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        .collect()
}

fn catalog_icon_url(project: &serde_json::Value) -> Option<String> {
    optional_string_field(project, &["icon_url"])
}

fn fetch_catalog_projects(
    client: &reqwest::blocking::Client,
    base_url: &str,
    project_ids: &[String],
) -> Result<HashMap<String, serde_json::Value>, String> {
    if project_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids = serde_json::to_string(project_ids).map_err(|error| error.to_string())?;
    let response = client
        .get(format!("{}/v3/projects", base_url.trim_end_matches('/')))
        .query(&[("ids", ids.as_str())])
        .send()
        .map_err(|error| format!("ShroudEdit is unreachable: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "ShroudEdit project lookup returned HTTP {}",
            response.status()
        ));
    }
    let projects = response
        .json::<serde_json::Value>()
        .map_err(|error| format!("Invalid ShroudEdit project response: {error}"))?;
    let projects = projects
        .as_array()
        .ok_or("ShroudEdit project response contains no project list")?;
    Ok(projects
        .iter()
        .filter_map(|project| {
            project
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(|id| (id.to_owned(), project.clone()))
        })
        .collect())
}

fn safe_filter_values(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| safe_filter_value(value))
        .filter(|value| !value.is_empty())
        .fold(Vec::new(), |mut unique, value| {
            if !unique.contains(&value) {
                unique.push(value);
            }
            unique
        })
}

fn catalog_search_facets_for_type(
    project_type: &str,
    loader: &[String],
    category: &[String],
    game_version: &[String],
    environment: &[String],
) -> Vec<Vec<String>> {
    let mut facets = vec![vec![format!("project_types:{project_type}")]];
    if project_type == "mod" {
        let selected_loaders = if loader.is_empty() {
            vec!["shroudforge", "eml"]
        } else {
            loader
                .iter()
                .map(String::as_str)
                .filter(|value| matches!(*value, "shroudforge" | "eml"))
                .collect()
        };
        if !selected_loaders.is_empty() {
            facets.push(
                selected_loaders
                    .into_iter()
                    .map(|value| format!("categories:{value}"))
                    .collect(),
            );
        }
    }
    if !category.is_empty() {
        facets.push(
            category
                .iter()
                .map(|value| format!("categories:{value}"))
                .collect(),
        );
    }
    if !game_version.is_empty() {
        facets.push(
            game_version
                .iter()
                .map(|value| format!("game_versions:{value}"))
                .collect(),
        );
    }
    if matches!(project_type, "mod" | "modpack") {
        let client = environment.iter().any(|value| value == "client");
        let server = environment.iter().any(|value| value == "server");
        let values: &[&str] = match (client, server) {
            (true, true) => &[
                "client_only_server_optional",
                "server_only_client_optional",
                "client_and_server",
                "client_or_server",
                "client_or_server_prefers_both",
            ],
            (true, false) => &[
                "client_only",
                "client_only_server_optional",
                "client_or_server_prefers_both",
                "client_or_server",
            ],
            (false, true) => &[
                "server_only",
                "dedicated_server_only",
                "server_only_client_optional",
                "client_or_server_prefers_both",
                "client_or_server",
            ],
            (false, false) => &[],
        };
        if !values.is_empty() {
            facets.push(
                values
                    .iter()
                    .map(|value| format!("environment:{value}"))
                    .collect(),
            );
        }
    }
    facets
}

fn string_values(value: Option<&serde_json::Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let values = value
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![value.clone()]);
    values
        .iter()
        .filter_map(|value| {
            value
                .as_str()
                .or_else(|| value.get("value").and_then(serde_json::Value::as_str))
                .or_else(|| value.get("name").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
        })
        .collect()
}

fn fetch_catalog_api_options(
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
) -> (CatalogApiOptions, bool) {
    const DISCOVER_TYPES: [&str; 4] = ["mod", "schematic", "modpack", "server"];
    let base = provider.base_url.trim_end_matches('/');
    let category_values = client
        .get(format!("{base}/v2/tag/category"))
        .send()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.json::<serde_json::Value>().ok())
        .and_then(|value| value.as_array().cloned());
    let game_version_values = client
        .get(format!("{base}/v2/tag/game_version"))
        .query(&[("type", "release")])
        .send()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.json::<serde_json::Value>().ok())
        .and_then(|value| value.as_array().cloned());
    let project_type_values = client
        .get(format!("{base}/v2/tag/project_type"))
        .send()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.json::<Vec<String>>().ok());

    let categories_by_project_type = category_values
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| {
            let project_type = value
                .get("project_type")
                .and_then(serde_json::Value::as_str)?;
            let name = value.get("name").and_then(serde_json::Value::as_str)?;
            if !matches!(project_type, "mod" | "schematic" | "modpack" | "server")
                || matches!(name.to_ascii_lowercase().as_str(), "eml" | "shroudforge")
            {
                return None;
            }
            Some((project_type.to_owned(), name.to_owned()))
        })
        .fold(
            HashMap::<String, Vec<String>>::new(),
            |mut categories, (kind, name)| {
                categories.entry(kind).or_default().push(name);
                categories
            },
        );
    let game_versions = game_version_values
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| {
            value
                .get("version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let supported = project_type_values.as_deref().unwrap_or_default();
    let project_types = DISCOVER_TYPES
        .into_iter()
        .filter(|project_type| supported.iter().any(|value| value == project_type))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let cacheable =
        category_values.is_some() && game_version_values.is_some() && project_type_values.is_some();
    (
        CatalogApiOptions {
            categories_by_project_type,
            game_versions,
            project_types,
        },
        cacheable,
    )
}

fn fetch_cached_catalog_api_options(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
) -> CatalogApiOptions {
    let options_key = api_cache::key(
        "options-v2",
        &provider.base_url,
        &serde_json::Value::Null,
    );
    let cached = api_cache::read::<CatalogApiOptions>(
        root,
        &options_key,
        CATALOG_OPTIONS_CACHE_SECONDS,
    );
    if let Some(options) = cached
        .as_ref()
        .filter(|options| catalog_api_options_are_complete(options))
    {
        return options.clone();
    }

    let (options, cacheable) = fetch_catalog_api_options(client, provider);
    if cacheable && catalog_api_options_are_complete(&options) {
        api_cache::write(root, options_key, &options);
        options
    } else {
        cached.unwrap_or(options)
    }
}

fn catalog_api_options_are_complete(options: &CatalogApiOptions) -> bool {
    !options.game_versions.is_empty()
        && !options.project_types.is_empty()
        && options.project_types.iter().all(|project_type| {
            options
                .categories_by_project_type
                .get(project_type)
                .is_some_and(|categories| !categories.is_empty())
        })
}

struct CatalogVersionSummary {
    number: String,
    environment: Option<String>,
}

fn fetch_version_summaries(
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    ids: &[String],
) -> std::collections::HashMap<String, CatalogVersionSummary> {
    if ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let Ok(ids) = serde_json::to_string(ids) else {
        return std::collections::HashMap::new();
    };
    client
        .get(format!(
            "{}/v3/versions",
            provider.base_url.trim_end_matches('/')
        ))
        .query(&[("ids", ids.as_str()), ("include_changelog", "false")])
        .send()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.json::<Vec<serde_json::Value>>().ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|version| {
            Some((
                version.get("id")?.as_str()?.to_owned(),
                CatalogVersionSummary {
                    number: version.get("version_number")?.as_str()?.to_owned(),
                    environment: optional_string_field(&version, &["environment"]),
                },
            ))
        })
        .collect()
}

pub fn fetch_catalog_project(
    root: &Path,
    provider: &CatalogProviderConfig,
    project_id: &str,
) -> Result<serde_json::Value, String> {
    if provider.kind != "shroudedit" {
        return Err(format!("unsupported catalog provider: {}", provider.kind));
    }
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err("invalid ShroudEdit project ID".into());
    }
    let base = provider.base_url.trim_end_matches('/');
    let detail_key = api_cache::key("project-detail", base, &serde_json::json!(project_id));
    if let Some(mut cached) =
        api_cache::read::<serde_json::Value>(root, &detail_key, CATALOG_DETAIL_CACHE_SECONDS)
    {
        let id = cached
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let slug = cached
            .get("slug")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        cached["installedModId"] = installed_mod_for_catalog_project(root, id, slug)
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        return Ok(cached);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(format!(
            "ShroudForge/{} (catalog-project)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let mut projects = fetch_catalog_projects(&client, base, &[project_id.to_owned()])?;
    let project = projects
        .remove(project_id)
        .ok_or_else(|| format!("ShroudEdit did not return project {project_id}"))?;
    let project_id = string_field(&project, &["id", "project_id"]);
    let advertised_versions = project
        .get("versions")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let mut versions = Vec::new();
    let mut offset = 0usize;
    loop {
        let offset_parameter = offset.to_string();
        let response = client
            .get(format!("{base}/v3/project/{project_id}/version"))
            .query(&[
                ("limit", "100"),
                ("offset", offset_parameter.as_str()),
                ("include_changelog", "true"),
            ])
            .send()
            .map_err(|error| format!("Could not read ShroudEdit versions: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "ShroudEdit version lookup returned HTTP {}",
                response.status()
            ));
        }
        let page = response
            .json::<serde_json::Value>()
            .map_err(|error| format!("Invalid ShroudEdit versions response: {error}"))?
            .as_array()
            .cloned()
            .unwrap_or_default();
        let count = page.len();
        versions.extend(page);
        if count < 100 || versions.len() >= advertised_versions {
            break;
        }
        offset += count;
    }
    let dependencies = client
        .get(format!("{base}/v3/project/{project_id}/dependencies"))
        .send()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.json::<serde_json::Value>().ok())
        .and_then(|value| {
            value
                .get("projects")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .unwrap_or_default()
        .iter()
        .map(|dependency| {
            serde_json::json!({
                "id": string_field(dependency, &["id", "project_id"]),
                "name": string_field(dependency, &["name", "title"]),
                "summary": string_field(dependency, &["summary"])
            })
        })
        .collect::<Vec<_>>();
    let gallery = project
        .get("gallery")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|image| {
            let url = image
                .as_str()
                .or_else(|| image.get("url").and_then(serde_json::Value::as_str))
                .or_else(|| image.get("raw_url").and_then(serde_json::Value::as_str))?;
            Some(serde_json::json!({
                "url": url,
                "rawUrl": image.get("raw_url").and_then(serde_json::Value::as_str).unwrap_or(url),
                "title": string_field(image, &["title"]),
                "description": string_field(image, &["description"]),
                "created": string_field(image, &["created"]),
            }))
        })
        .collect::<Vec<_>>();
    let links = project
        .get("link_urls")
        .or_else(|| project.get("links"))
        .and_then(serde_json::Value::as_object)
        .map(|links| {
            links
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .as_str()
                        .or_else(|| value.get("url").and_then(serde_json::Value::as_str))
                        .map(|url| (name.clone(), serde_json::Value::String(url.to_owned())))
                })
                .collect::<serde_json::Map<String, serde_json::Value>>()
        })
        .unwrap_or_default();
    let fields = project.get("fields");
    let game_versions = string_values(
        project
            .get("game_versions")
            .or_else(|| fields.and_then(|fields| fields.get("game_versions"))),
    );
    let versions = versions
        .iter()
        .map(|version| {
            serde_json::json!({
                "id": string_field(version, &["id"]),
                "name": string_field(version, &["name"]),
                "versionNumber": string_field(version, &["version_number"]),
                "published": string_field(version, &["date_published"]),
                "changelog": string_field(version, &["changelog"]),
                "versionType": string_field(version, &["version_type"]),
                "environment": optional_string_field(version, &["environment"]),
                "downloads": version.get("downloads").and_then(serde_json::Value::as_u64),
                "loaders": string_values(version.get("loaders")),
                "gameVersions": string_values(version.get("game_versions")),
                "projectTypes": string_values(version.get("project_types")),
                "files": version.get("files").cloned().unwrap_or_else(|| serde_json::json!([])),
                "dependencies": version.get("dependencies").cloned().unwrap_or_else(|| serde_json::json!([]))
            })
        })
        .collect::<Vec<_>>();
    let slug = optional_string_field(&project, &["slug"])
        .filter(|slug| !slug.is_empty())
        .unwrap_or_else(|| project_id.clone());
    let installed_mod_id = installed_mod_for_catalog_project(root, &project_id, &slug);
    let detail = serde_json::json!({
        "id": project_id,
        "slug": slug,
        "projectTypes": string_values(project.get("project_types")),
        "name": string_field(&project, &["name", "title"]),
        "summary": string_field(&project, &["summary"]),
        "description": string_field(&project, &["description"]),
        "downloads": project.get("downloads").and_then(serde_json::Value::as_u64),
        "follows": project.get("followers").or_else(|| project.get("follows")).and_then(serde_json::Value::as_u64),
        "dateModified": optional_string_field(&project, &["updated", "date_modified"]),
        "dateCreated": optional_string_field(&project, &["published", "date_created"]),
        "author": optional_string_field(&project, &["author", "owner_name"]),
        "iconUrl": catalog_icon_url(&project),
        "links": links,
        "loaders": string_values(project.get("loaders")),
        "categories": string_values(project.get("categories")),
        "gameVersions": game_versions,
        "gallery": gallery,
        "versionsCount": advertised_versions,
        "versions": versions,
        "dependencies": dependencies,
        "schematic": project.get("schematic").cloned().unwrap_or(serde_json::Value::Null),
        "enshroudedMod": project.get("enshrouded_mod").cloned().unwrap_or(serde_json::Value::Null),
        "enshroudedServer": project.get("enshrouded_server").cloned().unwrap_or(serde_json::Value::Null),
        "installedModId": installed_mod_id
    });
    api_cache::write(root, detail_key, &detail);
    Ok(detail)
}

pub fn read_catalog_install_registry(root: &Path) -> serde_json::Value {
    shroudforge_loader_config::config::read_document(root, crate::documents::CATALOG_STATE)
        .unwrap_or_else(|_| serde_json::json!({ "schemaVersion": 1, "projects": {}, "packs": {} }))
}

pub fn remember_catalog_pack_install(
    root: &Path,
    project_id: &str,
    pack: &InstalledCatalogPack,
) -> Result<(), String> {
    shroudforge_loader_config::config::update_state_section(root, "catalog", |stored| {
        let mut registry = stored
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"schemaVersion":1,"projects":{},"packs":{}}));
        if registry.get("packs").is_none() {
            registry["packs"] = serde_json::json!({});
        }
        registry["packs"][project_id] =
            serde_json::to_value(pack).map_err(|error| error.to_string())?;
        shroudforge_loader_config::config::validate_document(
            root,
            crate::documents::CATALOG_STATE,
            &registry,
        )?;
        Ok(registry)
    })
}

pub fn forget_catalog_pack_install(root: &Path, project_id: &str) -> Result<(), String> {
    shroudforge_loader_config::config::update_state_section(root, "catalog", |stored| {
        let mut registry = stored
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"schemaVersion":1,"projects":{},"packs":{}}));
        if registry.get("packs").is_none() {
            registry["packs"] = serde_json::json!({});
        }
        registry["packs"]
            .as_object_mut()
            .ok_or("invalid installed modpack registry")?
            .remove(project_id);
        shroudforge_loader_config::config::validate_document(
            root,
            crate::documents::CATALOG_STATE,
            &registry,
        )?;
        Ok(registry)
    })
}

pub fn catalog_installed_mod_ids(root: &Path) -> std::collections::HashSet<String> {
    read_catalog_install_registry(root)
        .get("projects")
        .and_then(|value| value.as_object())
        .into_iter()
        .flat_map(|projects| projects.values())
        .filter_map(|value| value.get("modId").and_then(|value| value.as_str()))
        .map(str::to_owned)
        .collect()
}

pub fn catalog_project_for_mod(root: &Path, mod_id: &str) -> Option<String> {
    read_catalog_install_registry(root)
        .get("projects")?
        .as_object()?
        .iter()
        .find_map(|(project_id, value)| {
            (value.get("modId").and_then(|value| value.as_str()) == Some(mod_id))
                .then(|| project_id.clone())
        })
}

fn installed_mod_for_catalog_project(root: &Path, project_id: &str, slug: &str) -> Option<String> {
    let registry = read_catalog_install_registry(root);
    let registered_mod_id = registry
        .pointer(&format!("/projects/{project_id}/modId"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    if let Some(mod_id) = registered_mod_id
        && shroudforge_package::package_lookup::find_mod_package(root, &mod_id).is_some()
    {
        return Some(mod_id);
    }
    if catalog_project_for_mod(root, slug).as_deref() == Some(project_id)
        || shroudforge_package::package_lookup::find_mod_package(root, slug).is_some()
    {
        return Some(slug.to_owned());
    }
    None
}

pub fn remember_catalog_install(
    root: &Path,
    project_id: &str,
    version_id: &str,
    mod_id: &str,
    package: &Path,
) -> Result<(), String> {
    shroudforge_loader_config::config::update_state_section(root, "catalog", |stored| {
        let mut registry = stored
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"schemaVersion":1,"projects":{},"packs":{}}));
        registry["projects"][project_id] = serde_json::json!({"modId":mod_id,"versionId":version_id,"file":package.file_name().and_then(|value|value.to_str()).unwrap_or_default()});
        shroudforge_loader_config::config::validate_document(
            root,
            crate::documents::CATALOG_STATE,
            &registry,
        )?;
        Ok(registry)
    })
}

pub fn forget_catalog_install(root: &Path, mod_id: &str) -> Result<(), String> {
    shroudforge_loader_config::config::update_state_section(root, "catalog", |stored| {
        let mut registry = stored
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"schemaVersion":1,"projects":{},"packs":{}}));
        registry["projects"]
            .as_object_mut()
            .ok_or("invalid catalog install registry")?
            .retain(|_, value| value.get("modId").and_then(|value| value.as_str()) != Some(mod_id));
        shroudforge_loader_config::config::validate_document(
            root,
            crate::documents::CATALOG_STATE,
            &registry,
        )?;
        Ok(registry)
    })
}

pub fn string_field(value: &serde_json::Value, names: &[&str]) -> String {
    optional_string_field(value, names).unwrap_or_default()
}

fn catalog_candidate_is_newer(candidate: &str, installed: &str, known_version_id: bool) -> bool {
    Version::parse(installed)
        .ok()
        .zip(Version::parse(candidate).ok())
        .is_some_and(|(current, next)| next > current || (known_version_id && next == current))
}

pub fn optional_string_field(value: &serde_json::Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        value
            .get(name)
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::{catalog_candidate_is_newer, catalog_search_facets_for_type};

    #[test]
    fn shrouedit_catalog_facets_use_the_project_types_field() {
        assert_eq!(
            catalog_search_facets_for_type("mod", &["shroudforge".into()], &[], &[], &[]),
            vec![
                vec!["project_types:mod".to_owned()],
                vec!["categories:shroudforge".to_owned()]
            ]
        );
        assert_eq!(
            catalog_search_facets_for_type(
                "mod",
                &[],
                &["technology".into()],
                &["1.0.1.2".into()],
                &[],
            ),
            vec![
                vec!["project_types:mod".to_owned()],
                vec![
                    "categories:shroudforge".to_owned(),
                    "categories:eml".to_owned()
                ],
                vec!["categories:technology".to_owned()],
                vec!["game_versions:1.0.1.2".to_owned()]
            ]
        );
    }

    #[test]
    fn environment_filters_match_discover_side_choices() {
        let client = catalog_search_facets_for_type("mod", &[], &[], &[], &["client".into()]);
        assert!(
            !client
                .last()
                .unwrap()
                .contains(&"environment:singleplayer_only".to_owned())
        );
        assert!(
            !client
                .last()
                .unwrap()
                .contains(&"environment:server_only".to_owned())
        );

        let both = catalog_search_facets_for_type(
            "modpack",
            &[],
            &[],
            &[],
            &["client".into(), "server".into()],
        );
        assert!(
            both.last()
                .unwrap()
                .contains(&"environment:client_and_server".to_owned())
        );
        assert!(
            !both
                .last()
                .unwrap()
                .contains(&"environment:client_only".to_owned())
        );
    }

    #[test]
    fn manual_mod_updates_only_offer_newer_semantic_versions() {
        assert!(!catalog_candidate_is_newer("3.2.0", "3.2.1", false));
        assert!(!catalog_candidate_is_newer("3.2.1", "3.2.1", false));
        assert!(catalog_candidate_is_newer("3.2.2", "3.2.1", false));
        assert!(catalog_candidate_is_newer("3.2.1", "3.2.1", true));
        assert!(!catalog_candidate_is_newer("invalid", "3.2.1", false));
    }
}
