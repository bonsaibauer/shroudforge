use crate::check_update_cancelled::check_update_cancelled;
use crate::write_update_status::{write_download_progress, write_status};
use std::{fs, io::Write, path::Path};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
const MAX_UPDATE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;

pub(crate) fn stage_system_update(root: &Path, request: &serde_json::Value) -> Result<(), String> {
    if request["installerAction"].as_bool() != Some(true)
        && !crate::update_check::updates_enabled(root)
    {
        return Err(crate::UPDATE_MODULE_DISABLED_MESSAGE.into());
    }
    let version = request["version"]
        .as_str()
        .ok_or("stage request has no version")?;
    let url = request["downloadUrl"]
        .as_str()
        .ok_or("stage request has no download URL")?;
    let checksum = request["checksum"]
        .as_str()
        .ok_or("stage request has no checksum")?
        .to_ascii_lowercase();
    if !url.starts_with("https://")
        || checksum.len() != 64
        || !checksum.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("stage request URL or SHA-256 checksum is invalid".into());
    }
    let expected_base_version = request["baseVersion"]
        .as_str()
        .ok_or("stage request has no base version")?;
    let expected_build = request["build"]
        .as_u64()
        .ok_or("stage request has no valid build number")?;
    let (tag_base_version, tag_build) = crate::parse_release_tag(version)?;
    if tag_base_version != expected_base_version || tag_build != expected_build {
        return Err("stage request version does not match its base version and build".into());
    }
    let updates = shroudforge_loader_config::paths::updates_dir(root);
    check_update_cancelled(root)?;
    fs::create_dir_all(&updates).map_err(|e| e.to_string())?;
    let download = updates.join("download.zip");
    let extraction = updates.join("pending-download");
    let pending = updates.join("pending");
    reset_extraction_directory(&extraction)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(15))
        .user_agent(format!("ShroudForge/{version} updater"))
        .build()
        .map_err(|e| e.to_string())?;
    let mut file = fs::File::create(&download).map_err(|e| e.to_string())?;
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    let mut total = 0u64;
    let mut last_progress = std::time::Instant::now();
    let mut last_bytes = 0u64;
    write_download_progress(root, version, 0, None, Some(0));
    let (_, content_length) = crate::download_response_body::download_response_body(
        root,
        client.get(url),
        MAX_UPDATE_BYTES,
        std::time::Duration::from_secs(30),
        "release download",
        "update package exceeds 2 GiB",
        |reported_length| {
            write_download_progress(root, version, 0, reported_length, Some(0));
            Ok(())
        },
        |chunk, content_length| {
            total = total.saturating_add(chunk.len() as u64);
            file.write_all(chunk).map_err(|error| error.to_string())?;
            hasher.update(chunk);
            if last_progress.elapsed() >= std::time::Duration::from_millis(300) {
                let elapsed = last_progress.elapsed().as_secs_f64().max(0.001);
                let speed = ((total - last_bytes) as f64 / elapsed) as u64;
                write_download_progress(root, version, total, content_length, Some(speed));
                last_progress = std::time::Instant::now();
                last_bytes = total;
            }
            Ok(())
        },
    )?;
    file.flush().map_err(|e| e.to_string())?;
    write_download_progress(
        root,
        version,
        total,
        content_length.or(Some(total)),
        Some(0),
    );
    write_status(
        root,
        "verifying",
        "Verifying the downloaded package checksum",
    );
    check_update_cancelled(root)?;
    if format!("{:x}", hasher.finalize()) != checksum {
        return Err("update package SHA-256 checksum does not match".into());
    }
    check_update_cancelled(root)?;
    write_status(root, "extracting", "Extracting and validating update files");
    let source = fs::File::open(&download).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(source).map_err(|e| e.to_string())?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("update archive contains too many entries".into());
    }
    let mut expanded = 0u64;
    for index in 0..archive.len() {
        check_update_cancelled(root)?;
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let entry_name = entry
            .name()
            .map_err(|error| error.to_string())?
            .into_owned();
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe archive path: {entry_name}"))?
            .to_path_buf();
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(format!("unsafe archive path: {entry_name}"));
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > 8 * MAX_UPDATE_BYTES {
            return Err("expanded update archive exceeds 16 GiB".into());
        }
        let output = extraction.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut file = fs::File::create(&output).map_err(|e| e.to_string())?;
            crate::apply_release_files::copy_reader_with_cancellation(
                &mut entry,
                &mut file,
                root,
                "update archive extraction",
            )?;
            file.flush().map_err(|e| e.to_string())?;
        }
    }
    check_update_cancelled(root)?;
    let manifest_path = extraction.join("shroudforge/version.json");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(&manifest_path)
            .map_err(|error| format!("update package has no version manifest: {error}"))?,
    )
    .map_err(|error| format!("update version manifest is invalid: {error}"))?;
    let manifest_version = manifest["version"]
        .as_str()
        .ok_or("update version manifest has no version")?;
    let manifest_build = manifest["build"]
        .as_u64()
        .or_else(|| {
            manifest["build"]
                .as_str()
                .and_then(|value| value.parse().ok())
        })
        .ok_or("update version manifest has no valid build number")?;
    if manifest_version.trim_start_matches('v') != expected_base_version.trim_start_matches('v')
        || manifest_build != expected_build
    {
        return Err("downloaded update version does not match the selected release".into());
    }
    crate::read_managed_release_files::managed_paths(&extraction)?;
    let updater_included = manifest["updaterIncluded"].as_bool().unwrap_or(true);
    if !updater_included
        && manifest["managedPaths"].as_array().is_some_and(|paths| {
            paths.iter().any(|path| {
                path.as_str().is_some_and(|path| {
                    path.eq_ignore_ascii_case("shroudforge/shroudforge-updater.exe")
                })
            })
        })
    {
        return Err("release manifest excludes the updater but lists it as a managed file".into());
    }
    for required in ["shroudforge/version.json", "shroudforge/shroudforge.exe"] {
        if !extraction.join(required).is_file() {
            return Err(format!("update package is missing {required}"));
        }
    }
    check_update_cancelled(root)?;
    if updater_included
        && !extraction
            .join("shroudforge/shroudforge-updater.exe")
            .is_file()
    {
        return Err("update package is missing shroudforge/shroudforge-updater.exe".into());
    }
    check_update_cancelled(root)?;
    let _ = fs::remove_file(updates.join("pending.ready"));
    check_update_cancelled(root)?;
    if pending.exists() {
        fs::remove_dir_all(&pending).map_err(|e| e.to_string())?;
    }
    check_update_cancelled(root)?;
    fs::rename(&extraction, &pending)
        .map_err(|e| format!("could not promote verified update: {e}"))?;
    let ready = serde_json::json!({"version":version,"checksumAlgorithm":"SHA-256","checksum":checksum,"stagedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
    shroudforge_loader_config::config::write_json(&updates.join("pending.ready"), &ready)?;
    let _ = fs::remove_file(download);
    Ok(())
}

fn reset_extraction_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "refusing to use a symbolic link as update staging: {}",
                    path.display()
                ));
            }
            #[cfg(windows)]
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(format!(
                    "refusing to use a reparse point as update staging: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                fs::remove_dir_all(path).map_err(|error| {
                    format!(
                        "could not clear old update staging {}: {error}",
                        path.display()
                    )
                })?;
            } else {
                fs::remove_file(path).map_err(|error| {
                    format!(
                        "could not remove old update staging {}: {error}",
                        path.display()
                    )
                })?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "could not inspect old update staging {}: {error}",
                path.display()
            ));
        }
    }
    fs::create_dir(path).map_err(|error| {
        format!(
            "could not create update staging {}: {error}",
            path.display()
        )
    })
}
