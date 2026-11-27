use crate::mod_package::ApiVersion;
use crate::provider_config::CatalogProviderConfig;
use crate::verify_mod_checksum::ModHashVerifier;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

struct TemporarySchematic {
    path: PathBuf,
    file: Option<File>,
}

impl TemporarySchematic {
    fn create(path: PathBuf) -> Result<Self, String> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("Could not create schematic download: {error}"))?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }

    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("schematic download is open")
    }

    fn close(&mut self) -> Result<(), String> {
        if let Some(file) = self.file.take() {
            file.sync_all()
                .map_err(|error| format!("Could not flush downloaded schematic: {error}"))?;
        }
        Ok(())
    }
}

impl Drop for TemporarySchematic {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

pub fn install_schematic_from_catalog(
    root: &Path,
    provider: &CatalogProviderConfig,
    project_id: &str,
    selected_version_id: Option<&str>,
    queue_item_id: &str,
) -> Result<Vec<String>, String> {
    shroudforge_schematics::require_enabled(root)?;
    if provider.kind != "shroudedit" {
        return Err("unsupported catalog provider".into());
    }
    if !valid_api_id(project_id) {
        return Err("invalid ShroudEdit project ID".into());
    }
    let version_id = selected_version_id.ok_or("select a schematic version to import")?;
    if !valid_api_id(version_id) {
        return Err("invalid ShroudEdit version ID".into());
    }
    crate::check_update_cancelled(root)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(format!(
            "ShroudForge/{} (schematic-importer)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let base = provider.base_url.trim_end_matches('/');
    let project_response = client
        .get(format!("{base}/v3/project/{project_id}"))
        .send()
        .map_err(|error| format!("ShroudEdit is unreachable: {error}"))?;
    crate::check_update_cancelled(root)?;
    if !project_response.status().is_success() {
        return Err(format!(
            "ShroudEdit project lookup returned HTTP {}",
            project_response.status()
        ));
    }
    let project: serde_json::Value = project_response
        .json()
        .map_err(|error| format!("Invalid ShroudEdit project response: {error}"))?;
    let returned_id = project
        .get("id")
        .or_else(|| project.get("project_id"))
        .and_then(serde_json::Value::as_str);
    if returned_id != Some(project_id) {
        return Err("ShroudEdit project response did not match the selected project".into());
    }
    let is_schematic = project
        .get("project_types")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|types| {
            types
                .iter()
                .any(|value| value.as_str() == Some("schematic"))
        });
    if !is_schematic {
        return Err("The selected ShroudEdit project is not a schematic".into());
    }

    let version_response = client
        .get(format!("{base}/v3/version/{version_id}"))
        .send()
        .map_err(|error| format!("Could not load schematic version metadata: {error}"))?;
    crate::check_update_cancelled(root)?;
    if !version_response.status().is_success() {
        return Err(format!(
            "ShroudEdit version lookup returned HTTP {}",
            version_response.status()
        ));
    }
    let version: ApiVersion = version_response
        .json()
        .map_err(|error| format!("Invalid ShroudEdit version response: {error}"))?;
    if version.id != version_id || version.project_id != project_id {
        return Err("Schematic version does not belong to the selected project".into());
    }
    let file = version
        .files
        .iter()
        .filter(|file| file.filename.to_ascii_lowercase().ends_with(".sfbp"))
        .find(|file| file.primary)
        .or_else(|| {
            version
                .files
                .iter()
                .find(|file| file.filename.to_ascii_lowercase().ends_with(".sfbp"))
        })
        .ok_or("This schematic version has no .sfbp blueprint file")?;
    if !file.url.starts_with("https://")
        || file.size == 0
        || file.size > shroudforge_schematics::validate_schematic_v8::MAX_BYTES as u64
    {
        return Err("Schematic file URL or size is invalid".into());
    }
    let mut hasher = ModHashVerifier::new(&file.hashes)?;
    let download_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(15))
        .user_agent(format!(
            "ShroudForge/{} (schematic-importer)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let staging_dir = shroudforge_loader_config::paths::updates_dir(root).join("schematic-staging");
    fs::create_dir_all(&staging_dir)
        .map_err(|error| format!("Could not prepare schematic download staging: {error}"))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = staging_dir.join(format!("schematic-{}-{stamp}.download", std::process::id()));
    let mut download = TemporarySchematic::create(temporary)?;
    crate::set_active_queue_item_state(
        root,
        queue_item_id,
        "downloading",
        Some("Downloading schematic blueprint"),
    )?;
    let (downloaded_bytes, _) = crate::download_response_body::download_response_body(
        root,
        download_client.get(&file.url),
        shroudforge_schematics::validate_schematic_v8::MAX_BYTES as u64,
        Duration::from_secs(30),
        "Schematic download",
        "Schematic exceeds 512 MiB",
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
    crate::check_update_cancelled(root)?;
    if downloaded_bytes != file.size {
        return Err("Downloaded schematic size does not match ShroudEdit version metadata".into());
    }
    download.close()?;
    hasher.verify()?;
    crate::check_update_cancelled(root)?;
    crate::set_active_queue_item_state(
        root,
        queue_item_id,
        "installing",
        Some("Blueprint download verified. Adding it to the schematic library"),
    )?;
    let title = project
        .get("title")
        .or_else(|| project.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(project_id);
    let downloaded_bytes = fs::read(&download.path)
        .map_err(|error| format!("Could not read verified schematic download: {error}"))?;
    let name = shroudforge_schematics::schematic_library::import_catalog_data(
        root,
        &downloaded_bytes,
        project_id,
        version_id,
        title,
    )?;
    Ok(vec![name])
}

fn valid_api_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}
