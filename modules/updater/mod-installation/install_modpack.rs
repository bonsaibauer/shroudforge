use crate::{mod_installation::catalog::{current_game_version, fetch_catalog_mod_versions, resolve_catalog_content_version}, provider_config::CatalogProviderConfig};
use shroudforge_package::modpack::{ModpackManifest, PackMod};
use std::{collections::{HashMap, HashSet}, fs, io::Write, path::{Path, PathBuf}, time::{Duration, SystemTime, UNIX_EPOCH}};

struct TemporaryPath(PathBuf);

impl Drop for TemporaryPath {
    fn drop(&mut self) {
        if self.0.is_dir() { let _ = fs::remove_dir_all(&self.0); }
        else { let _ = fs::remove_file(&self.0); }
    }
}

struct DownloadedPack {
    archive: TemporaryPath,
    manifest: ModpackManifest,
}

fn stamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
}

fn download_pack(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    version_id: &str,
) -> Result<DownloadedPack, String> {
    let base = provider.base_url.trim_end_matches('/');
    let response = client.get(format!("{base}/v3/version/{version_id}"))
        .send().map_err(|error| error.to_string())?;
    if !response.status().is_success() { return Err(format!("Modpack version lookup returned HTTP {}", response.status())) }
    let version: serde_json::Value = response.json().map_err(|error| error.to_string())?;
    if version["id"].as_str() != Some(version_id)
        || version["project_id"].as_str() != Some(project_id)
        || !version["project_types"].as_array().is_some_and(|items| items.iter().any(|item| item == "modpack"))
        || !version["loaders"].as_array().is_some_and(|items| items.iter().any(|item| item == "sfpack")) {
        return Err("selected version is not a ShroudForge modpack".into());
    }
    let file = version["files"].as_array()
        .and_then(|files| files.iter().find(|file| file["primary"] == true).or_else(|| files.first()))
        .ok_or("modpack version contains no archive")?;
    let url = file["url"].as_str().filter(|url| url.starts_with("https://"))
        .ok_or("modpack archive needs an HTTPS URL")?;
    if !file["filename"].as_str().is_some_and(|name| name.to_ascii_lowercase().ends_with(".sfpack"))
        || !file["size"].as_u64().is_some_and(|size| size > 0 && size <= crate::read_modpack::MAX_ARCHIVE_BYTES) {
        return Err("invalid modpack archive metadata".into());
    }
    let hashes: HashMap<String, String> = serde_json::from_value(file["hashes"].clone())
        .map_err(|error| error.to_string())?;
    if !hashes.get("sha512").is_some_and(|hash| hash.len() == 128 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())) {
        return Err("modpack archive needs a SHA-512 checksum".into());
    }
    let mut verifier = crate::verify_mod_checksum::ModHashVerifier::new(&hashes)?;
    let download_dir = shroudforge_loader_config::paths::ui_data_dir(root).join("downloads");
    fs::create_dir_all(&download_dir).map_err(|error| error.to_string())?;
    let archive = TemporaryPath(download_dir.join(format!("pack-{project_id}-{}.sfpack", stamp())));
    let mut output = fs::File::create(&archive.0).map_err(|error| error.to_string())?;
    let download_client = reqwest::Client::builder().timeout(Duration::from_secs(60))
        .user_agent(format!("ShroudForge/{} (modpack-installer)", env!("CARGO_PKG_VERSION")))
        .build().map_err(|error| error.to_string())?;
    crate::download_response_body::download_response_body(
        root, download_client.get(url), crate::read_modpack::MAX_ARCHIVE_BYTES,
        Duration::from_secs(30), "Modpack download", "Modpack exceeds 95 MiB",
        |_| Ok(()),
        |chunk, _| {
            output.write_all(chunk).map_err(|error| error.to_string())?;
            verifier.update(chunk);
            Ok(())
        },
    )?;
    output.flush().map_err(|error| error.to_string())?;
    drop(output);
    verifier.verify()?;
    let manifest = crate::read_modpack::read_modpack(fs::File::open(&archive.0).map_err(|error| error.to_string())?)?;
    if manifest.version != version["version_number"].as_str().unwrap_or_default() {
        return Err("modpack.json version differs from published version".into());
    }
    let slug = crate::install_catalog_mod::catalog_project_slug(client, provider, project_id)?;
    if manifest.id != slug { return Err("modpack.json id differs from ShroudEdit project slug".into()) }
    let mut published_games = version["game_versions"].as_array().into_iter().flatten()
        .filter_map(serde_json::Value::as_str).collect::<Vec<_>>();
    let mut pack_games = manifest.game_versions.iter().map(String::as_str).collect::<Vec<_>>();
    published_games.sort_unstable();
    pack_games.sort_unstable();
    if published_games != pack_games { return Err("modpack.json gameVersions differ from published version".into()) }
    if current_game_version(root).is_some_and(|game| !manifest.game_versions.contains(&game)) {
        return Err("modpack does not support the installed game version".into());
    }
    Ok(DownloadedPack { archive, manifest })
}

pub(crate) fn download_sfpack_manifest(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    version_id: &str,
) -> Result<ModpackManifest, String> {
    Ok(download_pack(root, client, provider, project_id, version_id)?.manifest)
}

pub(crate) fn install_modpack_from_catalog(
    root: &Path,
    provider: &CatalogProviderConfig,
    project_id: &str,
    selected_version_id: Option<&str>,
    queue_item_id: &str,
    optional_mods: Option<&[String]>,
) -> Result<Vec<String>, String> {
    if provider.kind != "shroudedit" { return Err("unsupported catalog provider".into()) }
    crate::check_update_cancelled(root)?;
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30))
        .user_agent(format!("ShroudForge/{} (modpack-installer)", env!("CARGO_PKG_VERSION")))
        .build().map_err(|error| error.to_string())?;
    let plan = resolve_catalog_content_version(root, &client, provider, project_id, &[], selected_version_id, "modpack")?;
    let downloaded = download_pack(root, &client, provider, project_id, &plan.primary.version_id)?;
    let pack = &downloaded.manifest;
    let previous = crate::mod_installation::catalog::installed_catalog_packs(root)
        .into_iter().find(|(id, _)| id == project_id).map(|(_, receipt)| receipt);
    let selected_optional = optional_mods.map(|mods| mods.to_vec())
        .or_else(|| previous.as_ref().map(|receipt| receipt.selected_optional_ids.iter()
            .filter(|id| pack.mods.iter().any(|item| item.id() == id.as_str() && item.optional()))
            .cloned().collect()))
        .unwrap_or_else(|| pack.mods.iter().filter(|item| item.optional()).map(|item| item.id().to_owned()).collect());
    if selected_optional.iter().collect::<HashSet<_>>().len() != selected_optional.len()
        || selected_optional.iter().any(|id| !pack.mods.iter().any(|item| item.id() == id && item.optional())) {
        return Err("optional mod selection contains a mod not declared as optional".into());
    }
    let mods_dir = shroudforge_loader_config::paths::mods_dir(root);
    let destination = mods_dir.join(&pack.id);
    if destination.exists() && !destination.join("modpack.json").is_file() {
        return Err(format!("mods/{} is occupied by another package", pack.id));
    }
    let staging_root = mods_dir.join(".shroudforge-staging");
    fs::create_dir_all(&staging_root).map_err(|error| error.to_string())?;
    let stage = TemporaryPath(staging_root.join(format!("pack-{}-{}", pack.id, stamp())));
    fs::create_dir(&stage.0).map_err(|error| error.to_string())?;
    crate::extract_mod_archive::extract_zip(root, &downloaded.archive.0, &stage.0)?;
    let package_dir = stage.0.join("mods");
    fs::create_dir_all(&package_dir).map_err(|error| error.to_string())?;
    let mut installed = Vec::new();
    let mut catalog_members = Vec::new();
    for item in &pack.mods {
        crate::check_update_cancelled(root)?;
        if item.optional() && !selected_optional.iter().any(|id| id == item.id()) {
            if let PackMod::Bundled { .. } = item {
                fs::remove_dir_all(package_dir.join(item.id())).map_err(|error| error.to_string())?;
            }
            continue
        }
        let id = item.id();
        let package = package_dir.join(id);
        match item {
            PackMod::Bundled { .. } => {},
            PackMod::Catalog { version, .. } => {
                let resolved = resolve_catalog_member(&client, provider, id, version, &pack.game_versions)?;
                let result = crate::download_mod_package::download_catalog_version_into(
                    root, provider, &client, &resolved, None, queue_item_id, Some(&package_dir))?;
                catalog_members.push(crate::mod_installation::catalog::InstalledPackMember {
                    project_id: resolved.project_id, version_id: resolved.version_id, mod_id: result.0,
                });
            }
            PackMod::External { version, url, sha512, .. } => {
                install_external(root, url, sha512, id, version, &package_dir)?;
            }
        }
        if !package.is_dir() { return Err(format!("modpack member {id} is missing")) }
        if package.join("mod.json").is_file() {
            let manifest = shroudforge_package::config::read_manifest_path(root, &package)?;
            if manifest.id != id { return Err(format!("modpack member {id} has a different mod.json id")) }
            if current_game_version(root).is_some_and(|game| !manifest.game_versions.is_empty() && !manifest.game_versions.contains(&game)) {
                return Err(format!("mod {id} does not support the installed game version"));
            }
        }
        installed.push(id.to_owned());
    }
    preserve_member_configuration(root, &destination, &package_dir, &installed)?;
    verify_pack_dependencies(root, &package_dir, &installed, &destination)?;
    let old_runtime = enabled_runtime_mods(root, &destination)?;
    let new_runtime = enabled_runtime_mods(root, &stage.0)?;
    let runtime_active = shroudforge_loader_config::runtime_status::runtime_is_active(root);
    if runtime_active && !old_runtime.is_empty() {
        crate::request_runtime_mod_unload(root, &old_runtime)?;
    }
    let backup = staging_root.join(format!("pack-{}-previous-{}", pack.id, stamp()));
    if destination.exists() {
        if let Err(error) = fs::rename(&destination, &backup) {
            if runtime_active && !old_runtime.is_empty() { let _ = crate::request_runtime_mod_reload(root, &old_runtime); }
            return Err(error.to_string());
        }
    }
    if let Err(error) = fs::rename(&stage.0, &destination) {
        if backup.exists() { let _ = fs::rename(&backup, &destination); }
        if runtime_active && !old_runtime.is_empty() { let _ = crate::request_runtime_mod_reload(root, &old_runtime); }
        return Err(error.to_string());
    }
    let receipt = crate::mod_installation::catalog::InstalledCatalogPack {
        pack_id: pack.id.clone(),
        version_id: plan.primary.version_id,
        version_number: pack.version.clone(),
        selected_optional_ids: selected_optional,
        mods: catalog_members,
    };
    let result = (|| {
        if runtime_active && !new_runtime.is_empty() {
            crate::request_runtime_mod_reload(root, &new_runtime)?;
        }
        crate::mod_installation::catalog::remember_catalog_pack_install(root, project_id, &receipt)
    })();
    if let Err(error) = result {
        let mut recovery_errors = Vec::new();
        if runtime_active && !new_runtime.is_empty() {
            if let Err(recovery) = crate::request_runtime_mod_unload(root, &new_runtime) { recovery_errors.push(recovery); }
        }
        let failed = fs::rename(&destination, &stage.0);
        let restored = if backup.exists() { fs::rename(&backup, &destination) } else { Ok(()) };
        if let Err(recovery) = failed { recovery_errors.push(format!("could not move failed pack aside: {recovery}")); }
        if let Err(recovery) = restored { recovery_errors.push(format!("could not restore previous pack: {recovery}")); }
        if runtime_active && !old_runtime.is_empty() {
            if let Err(recovery) = crate::request_runtime_mod_reload(root, &old_runtime) { recovery_errors.push(format!("could not reload previous mods: {recovery}")); }
        }
        return Err(if recovery_errors.is_empty() {
            format!("modpack installation failed; previous pack restored: {error}")
        } else {
            format!("modpack installation failed: {error}. Recovery failed: {}", recovery_errors.join("; "))
        });
    }
    if backup.exists() { let _ = fs::remove_dir_all(&backup); }
    Ok(installed)
}

pub(crate) fn enabled_runtime_mods(root: &Path, pack: &Path) -> Result<Vec<String>, String> {
    if !pack.is_dir() { return Ok(Vec::new()) }
    let mut active = Vec::new();
    let members = pack.join("mods");
    if !members.is_dir() { return Ok(active) }
    for entry in fs::read_dir(members).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().map_err(|error| error.to_string())?.is_dir() || !entry.path().join("mod.json").is_file() { continue }
        let manifest = shroudforge_package::config::read_manifest_path(root, &entry.path())?;
        if manifest.enabled && manifest.capabilities.iter().any(|capability| capability.requires_runtime()) {
            active.push(manifest.id);
        }
    }
    Ok(active)
}

fn preserve_member_configuration(root: &Path, previous: &Path, next: &Path, members: &[String]) -> Result<(), String> {
    for id in members {
        let old = previous.join("mods").join(id);
        let new = next.join(id);
        if !new.join("mod.json").is_file() { continue }
        let extension = old.join("extended.mod.json");
        let old_manifest = old.join("mod.json").is_file()
            .then(|| shroudforge_package::config::read_manifest_path(root, &old)).transpose()?;
        let old_value: Option<serde_json::Value> = extension.is_file()
            .then(|| fs::read(&extension).map_err(|error| error.to_string())
                .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string())))
            .transpose()?;
        let new_path = new.join("extended.mod.json");
        if !new_path.is_file() && old_value.is_none() { continue }
        let mut new_value: serde_json::Value = if new_path.is_file() {
            serde_json::from_slice(&fs::read(&new_path).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?
        } else {
            serde_json::json!({"schemaVersion":1})
        };
        let Some(object) = new_value.as_object_mut() else { return Err(format!("invalid extended.mod.json for {id}")) };
        object.insert("enabled".into(), old_manifest.as_ref().is_some_and(|manifest| manifest.enabled).into());
        if let Some(old_value) = &old_value {
            for key in ["targets", "settings", "inputReservations"] {
                if let Some(value) = old_value.get(key) { object.insert(key.into(), value.clone()); }
            }
        }
        shroudforge_loader_config::config::write_json(&new_path, &new_value)?;
        shroudforge_package::config::read_manifest_path(root, &new)?;
    }
    Ok(())
}

fn verify_pack_dependencies(root: &Path, package_dir: &Path, members: &[String], replaced_pack: &Path) -> Result<(), String> {
    let staged = members.iter().filter_map(|id| {
        let path = package_dir.join(id);
        path.join("mod.json").is_file().then_some(path)
    }).map(|path| shroudforge_package::config::read_manifest_path(root, &path))
        .collect::<Result<Vec<_>, _>>()?;
    for manifest in &staged {
        for dependency in &manifest.dependencies {
            if dependency.id == "shroudforge-api" { continue }
            let candidate = staged.iter().find(|item| item.id == dependency.id)
                .map(|item| item.version.clone())
                .or_else(|| {
                    let packages = shroudforge_package::modpack::package_locations(&shroudforge_loader_config::paths::mods_dir(root)).ok()?;
                    let matches = packages.into_iter().filter(|item| !item.path.starts_with(replaced_pack))
                        .filter_map(|item| shroudforge_package::config::read_manifest_path(root, &item.path).ok())
                        .filter(|item| item.id == dependency.id).collect::<Vec<_>>();
                    matches.iter().find(|item| item.enabled).or_else(|| matches.first()).map(|item| item.version.clone())
                });
            match candidate {
                Some(version) if !dependency.version.matches(&version) => return Err(format!("mod {} requires {} {}, found {}", manifest.id, dependency.id, dependency.version, version)),
                None if !dependency.optional.unwrap_or(false) => return Err(format!("mod {} requires missing mod {}", manifest.id, dependency.id)),
                _ => {}
            }
        }
    }
    Ok(())
}

fn resolve_catalog_member(
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    slug: &str,
    number: &str,
    games: &[String],
) -> Result<crate::mod_package::ResolvedContent, String> {
    let base = provider.base_url.trim_end_matches('/');
    let response = client.get(format!("{base}/v3/project/{slug}"))
        .send().map_err(|error| error.to_string())?;
    if !response.status().is_success() { return Err(format!("ShroudEdit project {slug} returned HTTP {}", response.status())) }
    let project: serde_json::Value = response.json().map_err(|error| error.to_string())?;
    if project["slug"].as_str() != Some(slug) { return Err(format!("ShroudEdit project slug differs from {slug}")) }
    if !project["project_types"].as_array().is_some_and(|types| types.iter().any(|kind| kind == "mod")) {
        return Err(format!("ShroudEdit project {slug} is not a mod"));
    }
    let project_id = project["id"].as_str().ok_or("ShroudEdit project has no ID")?;
    let versions = fetch_catalog_mod_versions(client, base, project_id, "[\"shroudforge\",\"eml\"]")?;
    let version = versions.iter().find(|item| item["version_number"].as_str() == Some(number)
        && matches!(item["status"].as_str(), Some("listed" | "archived" | "unlisted"))
        && item["game_versions"].as_array().is_some_and(|supported| games.iter().all(|game| supported.iter().any(|value| value == game)))
    ).ok_or_else(|| format!("ShroudEdit mod {slug} version {number} is unavailable"))?;
    Ok(crate::mod_package::ResolvedContent {
        project_id: project_id.into(),
        version_id: version["id"].as_str().ok_or("ShroudEdit mod version has no ID")?.into(),
    })
}

fn install_external(root: &Path, url: &str, sha512: &str, id: &str, version: &str, target: &Path) -> Result<(), String> {
    let url = reqwest::Url::parse(url).map_err(|error| format!("invalid external mod URL: {error}"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("external mod URL must be HTTPS without credentials".into());
    }
    let downloads = shroudforge_loader_config::paths::ui_data_dir(root).join("downloads");
    fs::create_dir_all(&downloads).map_err(|error| error.to_string())?;
    let archive = TemporaryPath(downloads.join(format!("external-{id}-{}.zip", stamp())));
    let mut output = fs::File::create(&archive.0).map_err(|error| error.to_string())?;
    let mut verifier = crate::verify_mod_checksum::ModHashVerifier::new(&HashMap::from([("sha512".into(), sha512.into())]))?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(60)).build().map_err(|error| error.to_string())?;
    crate::download_response_body::download_response_body(root, client.get(url), crate::mod_package::MAX_MOD_BYTES,
        Duration::from_secs(30), "External mod download", "External mod exceeds 512 MiB", |_| Ok(()),
        |chunk, _| {
            output.write_all(chunk).map_err(|error| error.to_string())?;
            verifier.update(chunk);
            Ok(())
        })?;
    output.flush().map_err(|error| error.to_string())?;
    drop(output);
    verifier.verify()?;
    let extraction = TemporaryPath(downloads.join(format!("external-{id}-{}-extract", stamp())));
    fs::create_dir(&extraction.0).map_err(|error| error.to_string())?;
    crate::extract_mod_archive::extract_zip(root, &archive.0, &extraction.0)?;
    let package = crate::download_mod_package::select_extracted_package(root, &extraction.0, Some(id))?;
    let manifest = shroudforge_package::config::read_manifest_path(root, &package)?;
    if manifest.id != id || manifest.version.to_string() != version {
        return Err(format!("external mod {id} does not match its declared ID and version"));
    }
    if current_game_version(root).is_some_and(|game| !manifest.game_versions.is_empty() && !manifest.game_versions.contains(&game)) {
        return Err(format!("external mod {id} does not support the installed game version"));
    }
    if target.exists() { return Err(format!("modpack destination for {id} already exists")) }
    crate::stage_mod_in_storage::stage_mod_in_storage(root, &package, id)?.commit(target)
}
