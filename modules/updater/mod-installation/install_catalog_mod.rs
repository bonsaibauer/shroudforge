use crate as shroudforge_updater;
use crate::download_mod_package::download_catalog_version;
use crate::mod_installation::catalog::{
    catalog_project_for_mod, forget_catalog_install, read_catalog_install_registry,
    remember_catalog_install, resolve_catalog_content_version, resolve_catalog_version,
};
use crate::provider_config::CatalogProviderConfig;
use crate::replace_installed_mod::replace_catalog_mod_with_runtime_dependencies;
use crate::verify_mod_checksum::verify_mod_hash;
use shroudforge_loader_config::runtime_status::runtime_is_active;
use shroudforge_package::package_lookup::find_mod_package;
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn install_catalog_content(
    root: &Path,
    provider: &CatalogProviderConfig,
    project_id: &str,
    selected_version_id: Option<&str>,
    update_mod_id: Option<&str>,
    queue_item_id: &str,
    content_type: &str,
) -> Result<Vec<String>, String> {
    match content_type {
        "mod" => install_mod_from_catalog(
            root,
            provider,
            project_id,
            selected_version_id,
            update_mod_id,
            queue_item_id,
        ),
        "modpack" if update_mod_id.is_none() => install_modpack_from_catalog(
            root, provider, project_id, selected_version_id, queue_item_id,
        ),
        "schematic" if update_mod_id.is_none() => crate::install_catalog_schematic::install_schematic_from_catalog(
            root,
            provider,
            project_id,
            selected_version_id,
            queue_item_id,
        ),
        "modpack" | "schematic" => Err("only mod projects can be updated".into()),
        _ => Err("unsupported ShroudEdit content type for installation".into()),
    }
}

pub fn install_modpack_from_catalog(
    root: &Path,
    provider: &CatalogProviderConfig,
    project_id: &str,
    selected_version_id: Option<&str>,
    queue_item_id: &str,
) -> Result<Vec<String>, String> {
    if provider.kind != "shroudedit" {
        return Err("unsupported catalog provider".into());
    }
    shroudforge_updater::check_update_cancelled(root)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(format!(
            "ShroudForge/{} (modpack-installer)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let registry = read_catalog_install_registry(root);
    let plan = resolve_catalog_content_version(
        root,
        &client,
        provider,
        project_id,
        &[],
        selected_version_id,
        "modpack",
    )?;
    if plan.primary.project_id != project_id {
        return Err("ShroudEdit resolved a different modpack project".into());
    }
    let pack = download_sfpack_manifest(root, &client, provider, project_id, &plan.primary.version_id)?;
    shroudforge_updater::check_update_cancelled(root)?;
    let mut installed_paths = Vec::new();
    let mut installed = Vec::new();
    let result = (|| {
        let mut all_package_paths = Vec::new();
        for item in &pack.mods {
            shroudforge_updater::check_update_cancelled(root)?;
            let slug = catalog_project_slug(&client, provider, &item.project_id)?;
            if let Some(local) = find_mod_package(root, &slug) {
                let recorded = registry["projects"][item.project_id.as_str()]
                    ["versionId"].as_str() == Some(item.version_id.as_str())
                    && registry["projects"][item.project_id.as_str()]["modId"].as_str() == Some(slug.as_str());
                if !recorded {
                    return Err(format!("Modpack requires exact catalog version {} of '{}', but a different local package is installed", item.version_id, slug));
                }
                all_package_paths.push(local);
                continue;
            }
            let dependency = crate::mod_package::ResolvedContent {
                project_id: item.project_id.clone(),
                version_id: item.version_id.clone(),
            };
            let package = download_catalog_version(
                root,
                provider,
                &client,
                &dependency,
                None,
                queue_item_id,
            )?;
            installed_paths.push(package.1.clone());
            all_package_paths.push(package.1.clone());
            remember_catalog_install(
                root,
                &dependency.project_id,
                &dependency.version_id,
                &package.0,
                &package.1,
            )?;
            installed.push(package.0);
        }
        shroudforge_updater::check_update_cancelled(root)?;
        verify_installed_mod_dependencies(root, &all_package_paths)?;
        let runtime_ids = installed
            .iter()
            .filter_map(|id| {
                let package = shroudforge_loader_config::paths::mods_dir(root).join(id);
                shroudforge_package::config::read_manifest_path(root, &package)
                    .ok()
                    .filter(|manifest| {
                        manifest
                            .capabilities
                            .iter()
                            .any(|capability| capability.requires_runtime())
                    })
                    .map(|manifest| manifest.id)
            })
            .collect::<Vec<_>>();
        if runtime_is_active(root) && !runtime_ids.is_empty() {
            shroudforge_updater::request_runtime_mod_reload(root, &runtime_ids).map_err(|error| {
                format!("modpack dependencies were installed, but runtime modules could not be loaded: {error}")
            })?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut cleanup_errors = Vec::new();
        let mut removed_mod_ids = Vec::new();
        for path in installed_paths {
            let mod_id = path.file_name().map(|name| name.to_string_lossy().into_owned());
            match move_directory_to_recycle_bin(root, &path, "Failed modpack installation") {
                Ok(()) => {
                    if let Some(mod_id) = mod_id {
                        removed_mod_ids.push(mod_id);
                    }
                }
                Err(cleanup_error) => cleanup_errors.push(cleanup_error),
            }
        }
        for mod_id in removed_mod_ids {
            if let Err(cleanup_error) = forget_catalog_install(root, &mod_id) {
                cleanup_errors.push(format!("could not clear catalog record for recycled mod {mod_id}: {cleanup_error}"));
            }
        }
        return Err(if cleanup_errors.is_empty() {
            error
        } else {
            format!("{error}. Cleanup also reported: {}", cleanup_errors.join("; "))
        });
    }
    Ok(installed)
}

fn download_sfpack_manifest(
    root: &Path,
    client: &reqwest::blocking::Client,
    provider: &CatalogProviderConfig,
    project_id: &str,
    version_id: &str,
) -> Result<crate::read_modpack::Modpack, String> {
    let base = provider.base_url.trim_end_matches('/');
    let response = client.get(format!("{base}/v3/version/{version_id}"))
        .send().map_err(|error| format!("Could not read modpack version: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Modpack version lookup returned HTTP {}", response.status()));
    }
    let version: serde_json::Value = response.json()
        .map_err(|error| format!("Invalid modpack version metadata: {error}"))?;
    if version["id"].as_str() != Some(version_id)
        || version["project_id"].as_str() != Some(project_id)
        || !version["project_types"].as_array().is_some_and(|types| types.iter().any(|value| value == "modpack"))
        || !version["loaders"].as_array().is_some_and(|loaders| loaders.iter().any(|value| value == "sfpack"))
    {
        return Err("Selected version is not a ShroudForge modpack".into());
    }
    let file = version["files"].as_array()
        .and_then(|files| files.iter().find(|file| file["primary"] == true).or_else(|| files.first()))
        .ok_or("Modpack version contains no archive")?;
    let url = file["url"].as_str().filter(|url| url.starts_with("https://"))
        .ok_or("Modpack archive has no HTTPS URL")?;
    if !file["filename"].as_str().is_some_and(|name| name.to_ascii_lowercase().ends_with(".sfpack"))
        || !file["size"].as_u64().is_some_and(|size| size > 0 && size <= 2 * 1024 * 1024)
    {
        return Err("Selected modpack archive is invalid".into());
    }
    let hashes = serde_json::from_value::<std::collections::HashMap<String, String>>(file["hashes"].clone())
        .map_err(|error| format!("Invalid modpack checksum metadata: {error}"))?;
    if !hashes.contains_key("sha512") {
        return Err("Modpack archive has no SHA-512 checksum".into());
    }
    shroudforge_updater::check_update_cancelled(root)?;
    let mut response = client.get(url).send()
        .map_err(|error| format!("Could not download .sfpack archive: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Modpack download returned HTTP {}", response.status()));
    }
    let mut bytes = Vec::new();
    (&mut response).take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read .sfpack archive: {error}"))?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("The .sfpack archive exceeds 2 MiB".into());
    }
    verify_mod_hash(&bytes, &hashes)?;
    let pack = crate::read_modpack::read_modpack(Cursor::new(bytes))?;
    if pack.version_id != version["version_number"].as_str().unwrap_or_default() {
        return Err(".sfpack versionId does not match its published version".into());
    }
    if let Some(game) = crate::mod_installation::catalog::current_game_version(root)
        && !pack.game_versions.contains(&game)
    {
        return Err(format!("Modpack does not support installed game version {game}"));
    }
    let mut published_games = version["game_versions"].as_array().into_iter().flatten()
        .filter_map(serde_json::Value::as_str).collect::<Vec<_>>();
    published_games.sort_unstable();
    let mut pack_games = pack.game_versions.iter().map(String::as_str).collect::<Vec<_>>();
    pack_games.sort_unstable();
    if published_games != pack_games {
        return Err(".sfpack gameVersions differ from the published version".into());
    }
    let mut pinned_versions = Vec::with_capacity(pack.mods.len());
    for item in &pack.mods {
        shroudforge_updater::check_update_cancelled(root)?;
        let response = client.get(format!("{base}/v3/version/{}", item.version_id))
            .send().map_err(|error| format!("Could not read pinned mod version: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Pinned mod version {} returned HTTP {}", item.version_id, response.status()));
        }
        pinned_versions.push(response.json::<serde_json::Value>()
            .map_err(|error| format!("Invalid pinned mod metadata: {error}"))?);
    }
    crate::read_modpack::validate_versions(&pack, &pinned_versions)?;
    Ok(pack)
}

pub fn install_mod_from_catalog(
    root: &Path,
    provider: &CatalogProviderConfig,
    project_id: &str,
    selected_version_id: Option<&str>,
    update_mod_id: Option<&str>,
    queue_item_id: &str,
) -> Result<Vec<String>, String> {
    if provider.kind != "shroudedit" {
        return Err("unsupported catalog provider".into());
    }
    shroudforge_updater::check_update_cancelled(root)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(format!(
            "ShroudForge/{} (mod-installer)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    if let Some(mod_id) = update_mod_id {
        if catalog_project_for_mod(root, mod_id).as_deref() != Some(project_id) {
            return Err(
                "catalog update target does not match the installed ShroudEdit project".into(),
            );
        }
        if find_mod_package(root, mod_id).is_none() {
            return Err(format!(
                "installed mod package '{mod_id}' could not be found"
            ));
        }
    }
    let plan = resolve_catalog_version(
        root,
        &client,
        provider,
        project_id,
        &[],
        selected_version_id,
    )?;
    shroudforge_updater::check_update_cancelled(root)?;
    let mut resolved = vec![plan.primary];
    resolved.extend(plan.dependencies);
    let mut installed_paths = Vec::new();
    let mut installed = Vec::new();
    let mut staged_update: Option<(String, String, PathBuf)> = None;
    let mut runtime_dependency_ids = Vec::new();
    let mut refresh_runtime_after_cleanup = false;
    let result = (|| {
        for item in resolved {
            shroudforge_updater::check_update_cancelled(root)?;
            let is_primary_update = update_mod_id.is_some() && item.project_id == project_id;
            if is_primary_update {
                let staged = download_catalog_version(
                    root,
                    provider,
                    &client,
                    &item,
                    update_mod_id,
                    queue_item_id,
                )?;
                staged_update = Some((item.project_id.clone(), item.version_id.clone(), staged.1));
                installed.push(staged.0);
                continue;
            }
			if item.project_id != project_id {
				let slug = catalog_project_slug(&client, provider, &item.project_id)?;
				if let Some(local_package) = find_mod_package(root, &slug) {
					let local = shroudforge_package::config::read_manifest_path(root, &local_package)
						.map_err(|error| format!("Could not read installed dependency {slug}: {error}"))?;
					let mut parent_packages = installed.iter()
						.filter(|id| Some(id.as_str()) != update_mod_id)
						.filter_map(|id| find_mod_package(root, id))
						.collect::<Vec<_>>();
					if let Some((_, _, staging)) = &staged_update {
						parent_packages.push(staging.clone());
					}
					if local_dependency_satisfies_plan(
						root, &client, provider, &item.project_id, &item.version_id,
						&local, &parent_packages,
					)? {
						continue;
					}
					return Err(format!(
						"Installed dependency '{}' at version {} does not satisfy the selected modpack. Update it before installing this mod.",
						local.id, local.version
					));
				}
			}
            let installed_mod =
                download_catalog_version(root, provider, &client, &item, None, queue_item_id)?;
            installed_paths.push(installed_mod.1.clone());
            remember_catalog_install(
                root,
                &item.project_id,
                &item.version_id,
                &installed_mod.0,
                &installed_mod.1,
            )?;
            installed.push(installed_mod.0);
        }
        shroudforge_updater::check_update_cancelled(root)?;
		let mut packages = installed
			.iter()
			.filter(|id| Some(id.as_str()) != update_mod_id)
			.filter_map(|id| find_mod_package(root, id))
			.collect::<Vec<_>>();
		if let Some((_, _, staging)) = &staged_update {
			packages.push(staging.clone());
		}
		verify_installed_mod_dependencies(root, &packages)?;
        runtime_dependency_ids = installed
            .iter()
            .filter(|id| Some(id.as_str()) != update_mod_id)
            .filter_map(|id| {
                let package = shroudforge_loader_config::paths::mods_dir(root).join(id);
                shroudforge_package::config::read_manifest_path(root, &package)
                    .ok()
                    .filter(|manifest| {
                        manifest
                            .capabilities
                            .iter()
                            .any(|capability| capability.requires_runtime())
                    })
                    .map(|manifest| manifest.id)
            })
            .collect();
        if let Some((updated_project_id, version_id, staging)) = staged_update.as_ref() {
            refresh_runtime_after_cleanup =
                runtime_is_active(root) && !runtime_dependency_ids.is_empty();
            let mod_id = update_mod_id.expect("staged update has target");
            let destination = shroudforge_loader_config::paths::mods_dir(root).join(mod_id);
            shroudforge_updater::check_update_cancelled(root)?;
            replace_catalog_mod_with_runtime_dependencies(
                root,
                mod_id,
                staging,
                &destination,
                updated_project_id,
                version_id,
                &runtime_dependency_ids,
            )?;
        } else if runtime_is_active(root) && !runtime_dependency_ids.is_empty() {
            shroudforge_updater::check_update_cancelled(root)?;
            shroudforge_updater::request_runtime_mod_reload(root, &runtime_dependency_ids)
                .map_err(|error| format!("catalog package installed, but its runtime modules could not be loaded in the running game: {error}"))?;
            shroudforge_updater::check_update_cancelled(root)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut cleanup_errors = Vec::new();
        let mut removed_mod_ids = Vec::new();
        for path in installed_paths {
            let mod_id = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            match move_directory_to_recycle_bin(root, &path, "Failed mod installation") {
                Ok(()) => {
                    if let Some(mod_id) = mod_id {
                        removed_mod_ids.push(mod_id);
                    }
                }
                Err(cleanup_error) => cleanup_errors.push(cleanup_error),
            }
        }
        if let Some((_, _, staging)) = staged_update
            && let Err(cleanup_error) =
                move_directory_to_recycle_bin(root, &staging, "Failed mod update staging files")
        {
            cleanup_errors.push(cleanup_error);
        }
        for mod_id in removed_mod_ids {
            if Some(mod_id.as_str()) != update_mod_id
                && let Err(cleanup_error) = forget_catalog_install(root, &mod_id)
            {
                cleanup_errors.push(format!(
                    "could not clear catalog record for recycled mod {mod_id}: {cleanup_error}"
                ));
            }
        }
        if refresh_runtime_after_cleanup
            && let Err(refresh_error) =
                shroudforge_updater::request_runtime_mod_reload(root, &runtime_dependency_ids)
        {
            cleanup_errors.push(format!(
                "runtime recovery after package cleanup also failed: {refresh_error}"
            ));
        }
        return Err(if cleanup_errors.is_empty() {
            error
        } else {
            format!(
                "{error}. Cleanup also reported: {}",
                cleanup_errors.join("; ")
            )
        });
    }
    Ok(installed)
}

pub(crate) fn catalog_project_slug(
	client: &reqwest::blocking::Client,
	provider: &CatalogProviderConfig,
	project_id: &str,
) -> Result<String, String> {
	let response = client.get(format!(
		"{}/v3/project/{project_id}", provider.base_url.trim_end_matches('/')
	)).send().map_err(|error| format!("Could not read ShroudEdit dependency project: {error}"))?;
	if !response.status().is_success() {
		return Err(format!("ShroudEdit dependency project {project_id} returned HTTP {}", response.status()));
	}
	let project: serde_json::Value = response.json()
		.map_err(|error| format!("Invalid ShroudEdit dependency project: {error}"))?;
	if project["id"].as_str() != Some(project_id) {
		return Err(format!("ShroudEdit returned a different project for {project_id}"));
	}
	project["slug"].as_str().filter(|slug| !slug.is_empty()).map(str::to_owned)
		.ok_or_else(|| format!("ShroudEdit project {project_id} has no mod slug"))
}

fn local_dependency_satisfies_plan(
	root: &Path,
	client: &reqwest::blocking::Client,
	provider: &CatalogProviderConfig,
	project_id: &str,
	version_id: &str,
	local: &shroudforge_package::ModManifest,
	parent_packages: &[PathBuf],
) -> Result<bool, String> {
	let mut declared = false;
	for path in parent_packages {
		let parent = shroudforge_package::config::read_manifest_path(root, path)
			.map_err(|error| format!("Could not read parent mod dependencies: {error}"))?;
		for dependency in parent.dependencies.iter().filter(|dependency| dependency.id == local.id) {
			if dependency.optional.unwrap_or(false) {
				continue;
			}
			declared = true;
			if !dependency.version.matches(&local.version) {
				return Ok(false);
			}
		}
	}
	if declared {
		return Ok(true);
	}
	let response = client.get(format!(
		"{}/v3/version/{version_id}", provider.base_url.trim_end_matches('/')
	)).send().map_err(|error| format!("Could not read ShroudEdit dependency version: {error}"))?;
	if !response.status().is_success() {
		return Err(format!("ShroudEdit dependency version {version_id} returned HTTP {}", response.status()));
	}
	let version: serde_json::Value = response.json()
		.map_err(|error| format!("Invalid ShroudEdit dependency version: {error}"))?;
	let local_version = local.version.to_string();
	Ok(version["id"].as_str() == Some(version_id)
		&& version["project_id"].as_str() == Some(project_id)
		&& version["version_number"].as_str() == Some(local_version.as_str()))
}

fn verify_installed_mod_dependencies(root: &Path, packages: &[PathBuf]) -> Result<(), String> {
	for package in packages {
		let manifest = shroudforge_package::config::read_manifest_path(root, package)
			.map_err(|error| format!("Could not read installed mod dependencies: {error}"))?;
		for dependency in &manifest.dependencies {
			if dependency.id == "shroudforge-api" {
				continue;
			}
			let candidate = find_mod_package(root, &dependency.id);
			let Some(candidate) = candidate else {
				if dependency.optional.unwrap_or(false) {
					continue;
				}
				return Err(format!(
					"Mod '{}' requires local mod '{}' ({}) but it is not installed",
					manifest.id, dependency.id, dependency.version
				));
			};
			let installed = shroudforge_package::config::read_manifest_path(root, &candidate)
				.map_err(|error| format!("Could not read dependency '{}': {error}", dependency.id))?;
			if !dependency.version.matches(&installed.version) {
				return Err(format!(
					"Mod '{}' requires '{}' ({}), but version {} is installed",
					manifest.id, dependency.id, dependency.version, installed.version
				));
			}
		}
	}
	Ok(())
}

fn move_directory_to_recycle_bin(root: &Path, source: &Path, reason: &str) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    let id = format!(
        "mod-cleanup-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let mods = shroudforge_loader_config::paths::mods_dir(root);
    let mod_id = source
        .parent()
        .filter(|parent| *parent == mods)
        .and_then(|_| source.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| shroudforge_package::package_lookup::valid_identifier(name));
    if let Some(mod_id) = mod_id {
        crate::recycle_bin::preserve_mod_directory(root, source, &id, reason, mod_id, None)?;
    } else {
        crate::recycle_bin::preserve_directory(root, source, &id, reason)?;
    }
    fs::remove_dir_all(source).map_err(|error| {
        format!(
            "could not remove recycled package {}: {error}",
            source.display()
        )
    })
}
