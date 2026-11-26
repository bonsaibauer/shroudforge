use crate as shroudforge_updater;
use crate::extract_mod_archive::extract_zip;
use crate::mod_package::{ApiVersion, MAX_MOD_BYTES, ResolvedContent};
use crate::provider_config::CatalogProviderConfig;
use crate::verify_mod_checksum::ModHashVerifier;
use shroudforge_package::package_lookup::find_mod_package;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct TemporaryDownload {
    path: PathBuf,
    file: Option<fs::File>,
}

impl TemporaryDownload {
    fn create(path: PathBuf) -> Result<Self, String> {
        let file = fs::File::create(&path).map_err(|error| error.to_string())?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }

    fn file_mut(&mut self) -> &mut fs::File {
        self.file.as_mut().expect("temporary download is open")
    }

    fn close(&mut self) {
        self.file.take();
    }
}

impl Drop for TemporaryDownload {
    fn drop(&mut self) {
        self.close();
        let _ = fs::remove_file(&self.path);
    }
}

struct StagingDirectory {
    path: PathBuf,
    keep: bool,
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub fn download_catalog_version(
    root: &Path,
    provider: &CatalogProviderConfig,
    client: &reqwest::blocking::Client,
    resolved: &ResolvedContent,
    update_mod_id: Option<&str>,
    queue_item_id: &str,
) -> Result<(String, PathBuf), String> {
    shroudforge_updater::check_update_cancelled(root)?;
    shroudforge_updater::set_active_queue_item_state(
        root,
        queue_item_id,
        "downloading",
        Some("Downloading mod package"),
    )?;
    let response = client
        .get(format!(
            "{}/v3/version/{}",
            provider.base_url.trim_end_matches('/'),
            resolved.version_id
        ))
        .send()
        .map_err(|error| format!("Could not load version metadata: {error}"))?;
    shroudforge_updater::check_update_cancelled(root)?;
    if !response.status().is_success() {
        return Err(format!(
            "Version metadata endpoint returned HTTP {}",
            response.status()
        ));
    }
    let version: ApiVersion = response
        .json()
        .map_err(|error| format!("Invalid version metadata: {error}"))?;
    if version.id != resolved.version_id || version.project_id != resolved.project_id {
        return Err("Version metadata does not belong to the resolved project".into());
    }
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or("Version contains no mod package")?;
    if !file.filename.to_ascii_lowercase().ends_with(".zip")
        || !file.url.starts_with("https://")
        || file.size > MAX_MOD_BYTES
    {
        return Err("Mod package is not a valid ZIP archive".into());
    }
    let download_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(15))
        .user_agent(format!(
            "ShroudForge/{} (mod-installer)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let downloads = shroudforge_loader_config::paths::ui_data_dir(root).join("downloads");
    fs::create_dir_all(&downloads).map_err(|error| error.to_string())?;
    let temporary = downloads.join(format!(
        "{}-{}-{}.zip",
        resolved.project_id,
        resolved.version_id,
        std::process::id()
    ));
    let mut download = TemporaryDownload::create(temporary.clone())?;
    let mut hasher = ModHashVerifier::new(&file.hashes)?;
    crate::download_response_body::download_response_body(
        root,
        download_client.get(&file.url),
        MAX_MOD_BYTES,
        Duration::from_secs(30),
        "Mod download",
        "Mod package exceeds 512 MiB",
        |_| Ok(()),
        |chunk, _| {
            download
                .file_mut()
                .write_all(chunk)
                .map_err(|error| error.to_string())?;
            hasher.update(chunk);
            Ok(())
        },
    )?;
    shroudforge_updater::check_update_cancelled(root)?;
    download
        .file_mut()
        .flush()
        .map_err(|error| error.to_string())?;
    download.close();
    hasher.verify()?;
    shroudforge_updater::check_update_cancelled(root)?;
    shroudforge_updater::set_active_queue_item_state(
        root,
        queue_item_id,
        "installing",
        Some("Download verified. Preparing mod installation"),
    )?;

    let mods = shroudforge_loader_config::paths::mods_dir(root);
    fs::create_dir_all(&mods).map_err(|error| error.to_string())?;
    let staging_root = if update_mod_id.is_some() {
        shroudforge_loader_config::paths::updates_dir(root).join("mod-staging")
    } else {
        downloads.clone()
    };
    fs::create_dir_all(&staging_root).map_err(|error| error.to_string())?;
    let transaction = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let extraction = staging_root.join(format!("{}-{transaction}-extract", std::process::id()));
    fs::create_dir(&extraction).map_err(|error| error.to_string())?;
    let extraction_cleanup = StagingDirectory {
        path: extraction.clone(),
        keep: false,
    };
    extract_zip(root, &temporary, &extraction)?;
    shroudforge_updater::check_update_cancelled(root)?;
    let package = select_extracted_package(root, &extraction, update_mod_id)?;
    let manifest = shroudforge_package::config::read_manifest_path(root, &package)
        .map_err(|error| format!("Invalid mod.json: {error}"))?;
    shroudforge_package::validate_manifest(&manifest)
        .map_err(|error| format!("Invalid mod manifest: {error}"))?;
    let project_slug =
        crate::install_catalog_mod::catalog_project_slug(client, provider, &resolved.project_id)?;
    if manifest.id != project_slug {
        return Err(format!(
            "mod.json ID '{}' differs from ShroudEdit project slug '{}'",
            manifest.id, project_slug
        ));
    }
    if manifest.version.to_string() != version._version_number {
        return Err(format!(
            "mod.json version '{}' differs from published version '{}'",
            manifest.version, version._version_number
        ));
    }
    if let Some(game_version) = crate::mod_installation::catalog::current_game_version(root)
        && !manifest.game_versions.is_empty()
        && !manifest.game_versions.contains(&game_version)
    {
        return Err(format!(
            "Mod '{}' does not support installed game version {game_version}",
            manifest.id
        ));
    }
    if update_mod_id.is_none() && find_mod_package(root, &manifest.id).is_some() {
        return Err(format!("Mod '{}' is already installed", manifest.name));
    }
    if let Some(expected_id) = update_mod_id
        && manifest.id != expected_id
    {
        return Err(format!(
            "catalog update package ID '{}' does not match installed mod '{expected_id}'",
            manifest.id
        ));
    }
    let profiles = shroudforge_package::mod_profiles::read_profiles(root)
        .map_err(|errors| errors.join("\n"))?;
    let profile = shroudforge_package::mod_profiles::select_profile(
        &profiles,
        &manifest.id,
        Some(&manifest.id),
        None,
    );
    if let Some(profile) = profile {
        import_profile_companions(root, &extraction, &package, profile)?;
        for file in &profile.files {
            let path = package.join(&file.path);
            crate::extract_mod_archive::ensure_child(&package, &path)?;
            if !path.is_file() {
                return Err(format!(
                    "Compatibility profile '{}' requires native component '{}'",
                    profile.name, file.path,
                ));
            }
        }
    }
    let native_entrypoint = profile.is_some_and(|profile| !profile.files.is_empty());
    let lua_entrypoint = package.join("src/mod.lua").is_file()
        || profile.is_some_and(|profile| profile.package_files.contains_key("src/mod.lua"));
    let ini_entrypoint = if lua_entrypoint || native_entrypoint {
        false
    } else {
        valid_native_ini_entrypoint(&package)?
    };
    if !lua_entrypoint && !native_entrypoint && !ini_entrypoint {
        return Err(
            "The mod has no src/mod.lua entrypoint, enabled native-plugin.ini or matching native compatibility profile"
                .into(),
        );
    }
    let staging = staging_root.join(format!("{}-{transaction}-package", manifest.id));
    fs::rename(&package, &staging)
        .map_err(|error| format!("Could not prepare the extracted mod folder: {error}"))?;
    let mut staging_cleanup = StagingDirectory {
        path: staging.clone(),
        keep: false,
    };
    // Only the selected package proceeds. Wrapper folders and unrelated root DLLs
    // belong to extraction and are removed by its guard.
    drop(extraction_cleanup);
    shroudforge_updater::check_update_cancelled(root)?;
    let destination = mods.join(&manifest.id);
    if let Some(expected_id) = update_mod_id {
        if manifest.id != expected_id {
            return Err("updated mod identity changed".into());
        }
        staging_cleanup.keep = true;
        Ok((manifest.id, staging))
    } else {
        if destination.exists() {
            return Err("mod destination already exists".into());
        }
        shroudforge_updater::check_update_cancelled(root)?;
        crate::stage_mod_in_storage::stage_mod_in_storage(root, &staging, &manifest.id)?
            .commit(&destination)?;
        Ok((manifest.id, destination))
    }
}

fn valid_native_ini_entrypoint(package: &Path) -> Result<bool, String> {
    let (state, dll) = shroudforge_package::dll_manager::read_plugin_ini(package);
    if state != "enabled" {
        return Ok(false);
    }
    let dll = dll.ok_or("Enabled native-plugin.ini must declare a DLL")?;
    let relative = Path::new(&dll);
    if relative
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)))
        || !dll.to_ascii_lowercase().ends_with(".dll")
    {
        return Err("native-plugin.ini must name a DLL inside the mod folder".into());
    }
    let package = package.canonicalize().map_err(|error| error.to_string())?;
    let path = package
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("native-plugin.ini DLL '{dll}' was not found: {error}"))?;
    if !path.starts_with(&package) || !path.is_file() {
        return Err("native-plugin.ini DLL must stay inside its mod folder".into());
    }
    Ok(true)
}

/// Archive layout belongs to installation. The running mod loader sees folders only.
fn select_extracted_package(
    root: &Path,
    extraction: &Path,
    expected_id: Option<&str>,
) -> Result<PathBuf, String> {
    let mut directories = vec![extraction.to_path_buf()];
    let mut packages = Vec::new();
    while let Some(directory) = directories.pop() {
        shroudforge_updater::check_update_cancelled(root)?;
        if directory.join("mod.json").is_file() {
            packages.push(directory.clone());
        }
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                directories.push(entry.path());
            }
        }
    }
    packages.sort();
    if let Some(expected_id) = expected_id {
        packages.retain(|package| {
            fs::read(package.join("mod.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_some_and(|manifest| manifest["id"].as_str() == Some(expected_id))
        });
    }
    match packages.as_slice() {
        [package] => Ok(package.clone()),
        [] if expected_id.is_some() => Err(format!("The archive contains no mod.json for the requested mod '{}'", expected_id.unwrap())),
        [] => Err("The archive contains no mod.json. For a native-only mod, put its own folder in mods and select its entry DLL in DLL Manager".into()),
        _ => Err("The archive contains multiple mod packages. Install one mod folder at a time".into()),
    }
}

fn import_profile_companions(
    root: &Path,
    extraction: &Path,
    package: &Path,
    profile: &shroudforge_package::mod_profiles::ModCompatibilityProfile,
) -> Result<(), String> {
    let mut mappings: Vec<_> = profile.import_files.iter().collect();
    mappings.sort_by_key(|(source, _)| *source);
    for (source, destination) in mappings {
        shroudforge_updater::check_update_cancelled(root)?;
        let target = package.join(destination);
        crate::extract_mod_archive::ensure_child(package, &target)?;
        if target.is_file() {
            continue;
        }
        let companion = package.ancestors().take_while(|ancestor| ancestor.starts_with(extraction))
            .map(|ancestor| ancestor.join(source)).find(|candidate| candidate.is_file())
            .ok_or_else(|| format!("Compatibility profile '{}' requires companion file '{source}' in the mod archive", profile.name))?;
        crate::extract_mod_archive::ensure_child(extraction, &companion)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut input = fs::File::open(&companion).map_err(|error| error.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| {
                format!("Could not import native companion '{destination}': {error}")
            })?;
        crate::apply_release_files::copy_reader_with_cancellation(
            &mut input,
            &mut output,
            root,
            "native mod companion import",
        )?;
        let _ = shroudforge_logging::append(
            root,
            'I',
            "mod-installation",
            &format!(
                "Imported native companion {source} into {}/{destination} using compatibility profile {}",
                profile.package_directory, profile.name,
            ),
        );
    }
    Ok(())
}
