use crate as shroudforge_updater;
use crate::validate_update_paths::{installed_path, validate_release_path};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use shroudforge_loader_config::runtime_status::runtime_is_active;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::SystemTime,
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

const DEFAULT_RETENTION_DAYS: u64 = 30;
static AUTOMATIC_SCHEMATIC_ATTEMPTS: LazyLock<Mutex<HashMap<PathBuf, (u64, Option<SystemTime>)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(windows)]
fn acquire_operation_lock(root: &Path) -> Result<crate::QueueWorkerMutex, String> {
    crate::acquire_queue_worker_mutex(root)?
        .ok_or_else(|| "an update or recovery action is already running".into())
}

#[cfg(not(windows))]
fn acquire_operation_lock(_: &Path) -> Result<(), String> {
    Ok(())
}

pub(crate) fn automatic_cleanup_enabled(root: &Path) -> bool {
    shroudforge_loader_config::config::read_loader(root)
        .ok()
        .and_then(|config| {
            config
                .pointer("/modules/updates/recycleBin/automaticCleanup")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(true)
}

fn retention_seconds(root: &Path) -> u64 {
    let days = shroudforge_loader_config::config::read_loader(root)
        .ok()
        .and_then(|config| {
            config
                .pointer("/modules/updates/recycleBin/retentionDays")
                .and_then(serde_json::Value::as_u64)
        })
        .unwrap_or(DEFAULT_RETENTION_DAYS)
        .clamp(1, 3650);
    days.saturating_mul(24 * 60 * 60)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EntryManifest {
    schema_version: u32,
    id: String,
    reason: String,
    created_at: u64,
    expires_at: u64,
    files: Vec<StoredFile>,
    #[serde(default)]
    restore_paths: Vec<String>,
    #[serde(default)]
    restore_mod_id: Option<String>,
    #[serde(default)]
    restore_root_files: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    restore_schematic_name: Option<String>,
    #[serde(default)]
    catalog_project_id: Option<String>,
    #[serde(default)]
    catalog_version_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredFile {
    path: String,
    size: u64,
    sha256: String,
}

struct RestoreOptions {
    mod_id: Option<String>,
    root_files: bool,
    schematic_name: Option<String>,
    catalog_entry: Option<(String, String)>,
    paths: Option<Vec<String>>,
}

/// Preserve the installed files needed by an update transaction in the one
/// ShroudForge recycle bin. The returned directory keeps the existing
/// rollback implementation usable without a second backup format.
pub(crate) fn preserve_update_files(
    root: &Path,
    id: &str,
    reason: &str,
    paths: &[String],
) -> Result<std::path::PathBuf, String> {
    preserve_update_files_with_progress_and_cancel(
        root,
        id,
        reason,
        paths,
        None,
        |_, _, _, _, _| Ok(()),
    )
}

pub(crate) fn preserve_update_files_with_progress_and_cancel(
    root: &Path,
    id: &str,
    reason: &str,
    paths: &[String],
    cancel_root: Option<&Path>,
    mut progress: impl FnMut(usize, usize, &str, u64, u64) -> Result<(), String>,
) -> Result<std::path::PathBuf, String> {
    validate_entry_id(id)?;

    let entry = shroudforge_loader_config::paths::recycle_bin_dir(root).join(id);
    let files = entry.join("files");
    let directory = shroudforge_loader_config::paths::recycle_bin_dir(root);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    fs::create_dir(&entry)
        .map_err(|error| format!("could not create recycle-bin entry {id}: {error}"))?;
    let result = (|| {
        fs::create_dir(&files).map_err(|error| error.to_string())?;
        let mut preserved = Vec::new();
        for (index, relative) in paths.iter().enumerate() {
            validate_release_path(relative)?;
            let source = installed_path(root, relative);
            let metadata = match fs::symlink_metadata(&source) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    progress(index + 1, paths.len(), "", 0, 0)?;
                    continue;
                }
                Err(error) => {
                    return Err(format!(
                        "could not inspect update target {} before preserving it: {error}",
                        source.display()
                    ));
                }
            };
            if is_reparse_point(&metadata) {
                return Err(format!(
                    "update target became a reparse point before it could be preserved: {relative}"
                ));
            }
            if !metadata.is_file() {
                return Err(format!("update target is not a file: {relative}"));
            }
            let destination = files.join(validate_release_path(relative)?);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            preserved.push(copy_verified_file_with_progress_and_cancel(
                &source,
                &destination,
                relative,
                cancel_root,
                |bytes, total| progress(index, paths.len(), relative, bytes, total),
            )?);
            progress(index + 1, paths.len(), "", 0, 0)?;
        }
        write_manifest(
            root,
            &entry,
            id,
            reason,
            preserved,
            RestoreOptions {
                mod_id: None,
                root_files: true,
                schematic_name: None,
                catalog_entry: None,
                paths: Some(paths.to_vec()),
            },
        )?;
        Ok(files.clone())
    })();
    result.map_err(|error| remove_failed_entry(Some(root), &entry, error))
}

pub(crate) fn convert_schematic_to_latest(root: &Path, name: &str) -> Result<(), String> {
    let _operation_lock = acquire_operation_lock(root)?;
    ensure_queue_idle(
        root,
        "schematics cannot be converted while an update is running",
    )?;
    if !shroudforge_schematics::schematic_library::valid_name(name) {
        return Err("invalid schematic name".into());
    }
    let schematic = shroudforge_schematics::schematic_library::list(root)?
        .into_iter()
        .find(|schematic| schematic.name == name)
        .ok_or("schematic was not found")?;
    if schematic.valid {
        return Ok(());
    }
    if !schematic.convertible || schematic.format_version != Some(7) {
        return Err(schematic
            .validation_error
            .unwrap_or_else(|| "schematic cannot be converted to the latest format".into()));
    }

    let library = shroudforge_schematics::schematic_library::directory(root);
    let source = library.join(format!("{name}.sfbp"));
    let source_metadata = fs::symlink_metadata(&source)
        .map_err(|error| format!("could not inspect schematic: {error}"))?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
        return Err("schematic must be a regular file".into());
    }
    if source_metadata.len() > shroudforge_schematics::validate_schematic_v8::MAX_BYTES as u64 {
        return Err("schematic exceeds the 256 MiB size limit".into());
    }
    let original =
        fs::read(&source).map_err(|error| format!("could not read schematic: {error}"))?;
    let inspection = shroudforge_schematics::convert_schematic_v7::inspect(&original);
    if !inspection.convertible || inspection.format_version != Some(7) {
        return Err(inspection
            .validation_error
            .unwrap_or_else(|| "schematic changed before conversion".into()));
    }
    let converted = shroudforge_schematics::convert_schematic_v7::convert_v7_to_v8(&original)?;

    let id = format!("schematic-v7-{}", unique_stamp());
    let entry = shroudforge_loader_config::paths::recycle_bin_dir(root).join(&id);
    let files = entry.join("files");
    fs::create_dir_all(entry.parent().ok_or("recycle-bin entry has no parent")?)
        .map_err(|error| error.to_string())?;
    fs::create_dir(&entry)
        .map_err(|error| format!("could not create recycle-bin entry {id}: {error}"))?;
    let saved = files.join(format!("{name}.sfbp"));
    let prepared = (|| {
        fs::create_dir(&files).map_err(|error| error.to_string())?;
        let relative = format!("{name}.sfbp");
        let before = stored_file_with_cancellation(&source, &relative, None)?;
        let copied = copy_verified_file(&source, &saved, &relative)?;
        if copied != before || stored_file_with_cancellation(&source, &relative, None)? != before {
            return Err("schematic changed while it was moved to the recycle bin".into());
        }
        fs::remove_file(&source)
            .map_err(|error| format!("could not move v7 schematic to the recycle bin: {error}"))?;
        write_manifest(
            root,
            &entry,
            &id,
            &format!("SFBP v7 from {name}"),
            vec![copied],
            RestoreOptions {
                mod_id: None,
                root_files: false,
                schematic_name: Some(name.to_owned()),
                catalog_entry: None,
                paths: None,
            },
        )
    })();
    if let Err(error) = prepared {
        let recovery = if saved.exists() && !source.exists() {
            restore_recycled_schematic_file(&saved, &source, &format!("{name}.sfbp"))
        } else {
            Ok(())
        };
        return match recovery {
            Ok(()) => {
                let _ = fs::remove_dir_all(&entry);
                Err(error)
            }
            Err(recovery_error) => {
                if let Ok(record) =
                    stored_file_with_cancellation(&saved, &format!("{name}.sfbp"), None)
                {
                    let _ = write_manifest(
                        root,
                        &entry,
                        &id,
                        &format!("SFBP v7 from {name}"),
                        vec![record],
                        RestoreOptions {
                            mod_id: None,
                            root_files: false,
                            schematic_name: Some(name.to_owned()),
                            catalog_entry: None,
                            paths: None,
                        },
                    );
                }
                Err(format!(
                    "{error}. The original v7 file remains in recycle-bin entry {id}: {recovery_error}"
                ))
            }
        };
    }

    if let Err(error) = shroudforge_schematics::schematic_library::save(root, name, &converted) {
        if source.exists() {
            return Err(format!(
                "conversion failed and the original v7 file remains in recycle-bin entry {id}: {error}"
            ));
        }
        match restore_recycled_schematic_file(&saved, &source, &format!("{name}.sfbp")) {
            Ok(()) => {
                let _ = fs::remove_dir_all(&entry);
                return Err(error);
            }
            Err(recovery_error) => {
                return Err(format!(
                    "{error}. The original v7 file remains recoverable in recycle-bin entry {id}: {recovery_error}"
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn convert_automatic_schematics(root: &Path) -> Result<(), String> {
    let config = shroudforge_loader_config::config::read_loader(root)?;
    if config
        .pointer("/modules/schematics/conversionMode")
        .and_then(serde_json::Value::as_str)
        != Some("automatic")
    {
        return Ok(());
    }
    let schematics = shroudforge_schematics::schematic_library::list(root)?;
    let library = shroudforge_schematics::schematic_library::directory(root);
    let mut errors = Vec::new();
    for schematic in schematics.iter().filter(|item| item.convertible) {
        let path = library.join(format!("{}.sfbp", schematic.name));
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let signature = (metadata.len(), metadata.modified().ok());
        if AUTOMATIC_SCHEMATIC_ATTEMPTS
            .lock()
            .ok()
            .and_then(|attempts| attempts.get(&path).cloned())
            .is_some_and(|previous| previous == signature)
        {
            continue;
        }
        if let Err(error) = convert_schematic_to_latest(root, &schematic.name) {
            errors.push(format!("{}: {error}", schematic.name));
            if !error.contains("while an update is running")
                && !error.contains("already running")
                && let Ok(mut attempts) = AUTOMATIC_SCHEMATIC_ATTEMPTS.lock()
            {
                attempts.insert(path, signature);
                if attempts.len() > 2048 {
                    attempts.clear();
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(". "))
    }
}

fn restore_recycled_schematic_file(
    saved: &Path,
    destination: &Path,
    relative: &str,
) -> Result<(), String> {
    if destination.exists() {
        return Err("schematic path is occupied and cannot be rolled back".into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    copy_verified_file(saved, destination, relative)?;
    fs::remove_file(saved).map_err(|error| error.to_string())
}

pub(crate) fn preserve_directory(
    root: &Path,
    source: &Path,
    id: &str,
    reason: &str,
) -> Result<std::path::PathBuf, String> {
    preserve_directory_with_restore(root, source, id, reason, None, None, None)
}

pub(crate) fn recycle_game_asset_backups(root: &Path, target: &str) -> Result<String, String> {
    let _operation_lock = acquire_operation_lock(root)?;
    ensure_queue_idle(
        root,
        "game asset backups cannot be recycled while an update is running",
    )?;
    let stem = match target {
        "enshrouded" | "client" => "enshrouded",
        "enshrouded_server" | "server" => "enshrouded_server",
        _ => return Err("unknown game target for asset backup recycling".into()),
    };
    let id = format!("game-assets-{stem}-{}", unique_stamp());
    let stage = std::env::temp_dir().join(format!("shroudforge-{id}"));
    fs::create_dir(&stage).map_err(|error| error.to_string())?;
    let names = [
        format!("{stem}.kfc.bak"),
        format!("{stem}.kfc_resources.bak"),
    ];
    let result = (|| {
        let mut found = Vec::new();
        for name in &names {
            let source = root.join(name);
            let metadata = match fs::symlink_metadata(&source) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.to_string()),
            };
            if is_reparse_point(&metadata) || !metadata.is_file() {
                return Err(format!(
                    "working asset backup is not a regular file: {name}"
                ));
            }
            copy_verified_file_with_progress_and_cancel(
                &source,
                &stage.join(name),
                name,
                None,
                |_, _| Ok(()),
            )?;
            found.push(source);
        }
        if found.is_empty() {
            return Err("there are no working asset backups to move to the recycle bin".into());
        }
        preserve_directory(root, &stage, &id, "Incompatible game asset working backups")?;
        for source in found {
            fs::remove_file(&source).map_err(|error| {
                format!(
                    "asset backups were saved in the recycle bin but could not remove {}: {error}",
                    source.display()
                )
            })?;
        }
        Ok(id.clone())
    })();
    let _ = fs::remove_dir_all(&stage);
    result
}

pub(crate) fn preserve_mod_directory(
    root: &Path,
    source: &Path,
    id: &str,
    reason: &str,
    mod_id: &str,
    catalog_entry: Option<(String, String)>,
) -> Result<std::path::PathBuf, String> {
    preserve_mod_directory_with_cancellation(root, source, id, reason, mod_id, catalog_entry, None)
}

pub(crate) fn preserve_mod_directory_with_cancellation(
    root: &Path,
    source: &Path,
    id: &str,
    reason: &str,
    mod_id: &str,
    catalog_entry: Option<(String, String)>,
    cancel_root: Option<&Path>,
) -> Result<std::path::PathBuf, String> {
    if !shroudforge_package::package_lookup::valid_identifier(mod_id) {
        return Err("invalid mod id for recycle-bin entry".into());
    }
    preserve_directory_with_restore(
        root,
        source,
        id,
        reason,
        Some(mod_id),
        catalog_entry,
        cancel_root,
    )
}

fn preserve_directory_with_restore(
    root: &Path,
    source: &Path,
    id: &str,
    reason: &str,
    restore_mod_id: Option<&str>,
    catalog_entry: Option<(String, String)>,
    cancel_root: Option<&Path>,
) -> Result<std::path::PathBuf, String> {
    validate_entry_id(id)?;
    let source_kind = fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if source_kind.file_type().is_symlink() || !source_kind.is_dir() {
        return Err(format!(
            "recycle-bin source is not a directory: {}",
            source.display()
        ));
    }
    let entry = shroudforge_loader_config::paths::recycle_bin_dir(root).join(id);
    let files = entry.join("files");
    fs::create_dir_all(entry.parent().ok_or("recycle-bin entry has no parent")?)
        .map_err(|error| error.to_string())?;
    fs::create_dir(&entry)
        .map_err(|error| format!("could not create recycle-bin entry {id}: {error}"))?;
    let result = (|| {
        fs::create_dir(&files).map_err(|error| error.to_string())?;
        let records = copy_backup_tree_with_cancel(source, source, &files, cancel_root)?;
        if records != read_file_records(source)? {
            return Err(format!("recycle-bin copy verification failed for {id}"));
        }
        write_manifest(
            root,
            &entry,
            id,
            reason,
            records,
            RestoreOptions {
                mod_id: restore_mod_id.map(str::to_owned),
                root_files: false,
                schematic_name: None,
                catalog_entry,
                paths: None,
            },
        )?;
        Ok(files.clone())
    })();
    result.map_err(|error| remove_failed_entry(Some(root), &entry, error))
}

pub(crate) fn restore_directory(saved: &Path, destination: &Path) -> Result<(), String> {
    restore_directory_with_cancellation(saved, destination, None)
}

pub(crate) fn restore_directory_with_cancellation(
    saved: &Path,
    destination: &Path,
    cancel_root: Option<&Path>,
) -> Result<(), String> {
    let saved_kind = fs::symlink_metadata(saved).map_err(|error| error.to_string())?;
    if saved_kind.file_type().is_symlink() || !saved_kind.is_dir() {
        return Err(format!(
            "recycle-bin package is not a regular directory: {}",
            saved.display()
        ));
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err(format!(
            "cannot restore package because the target already exists: {}",
            destination.display()
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let result = (|| {
        let copied_records = copy_backup_tree_with_cancel(saved, saved, destination, cancel_root)?;
        if read_file_records(saved)? != copied_records {
            return Err(format!(
                "restored package verification failed: {}",
                destination.display()
            ));
        }
        Ok(())
    })();
    result.map_err(|error| remove_failed_entry(None, destination, error))
}

fn remove_failed_entry(root: Option<&Path>, entry: &Path, operation_error: String) -> String {
    match fs::remove_dir_all(entry) {
        Ok(()) => operation_error,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => operation_error,
        Err(cleanup_error) => {
            let detail = format!(
                "{operation_error}. Could not clean up partial data at {}: {cleanup_error}",
                entry.display()
            );
            if let Some(root) = root {
                let _ = shroudforge_logging::append(root, 'E', "updater", &detail);
            }
            detail
        }
    }
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub(crate) fn migrate_legacy_backups(root: &Path) -> Result<usize, String> {
    let sources = [
        (
            shroudforge_loader_config::paths::updates_dir(root).join("backups"),
            "legacy-update-backup-",
            "Migrated legacy updater backup",
        ),
        (
            root.join("Shroudforge_Updates").join("backups"),
            "legacy-update-backup-old-layout-",
            "Migrated legacy updater backup from the pre-1.3.3 storage layout",
        ),
        (
            shroudforge_loader_config::paths::updates_dir(root).join("mod-backups"),
            "legacy-mod-backup-",
            "Migrated legacy mod backup",
        ),
        (
            shroudforge_loader_config::paths::ui_data_dir(root).join("trash"),
            "legacy-removed-mod-",
            "Migrated legacy removed mod",
        ),
        (
            root.join("Shroudforge_UI").join("trash"),
            "legacy-removed-mod-old-layout-",
            "Migrated legacy removed mod from the pre-1.3.3 storage layout",
        ),
    ];
    let mut migrated = 0;
    for (source_root, prefix, reason) in sources {
        crate::check_update_cancelled(root)?;
        migrated += migrate_legacy_directory(root, &source_root, prefix, reason)?;
    }
    Ok(migrated)
}

fn migrate_legacy_directory(
    root: &Path,
    source_root: &Path,
    prefix: &str,
    reason: &str,
) -> Result<usize, String> {
    if !source_root.is_dir() {
        return Ok(0);
    }
    let mut entries = fs::read_dir(source_root)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut migrated = 0;
    for entry in entries {
        crate::check_update_cancelled(root)?;
        let source = entry.path();
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_symlink() {
            return Err(format!(
                "legacy storage contains a symbolic link: {}",
                source.display()
            ));
        }
        if !kind.is_file() && !kind.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let id = format!(
            "{prefix}{:x}",
            sha2::Sha256::digest(format!("{prefix}{name}").as_bytes())
        );
        validate_entry_id(&id)?;
        let target = shroudforge_loader_config::paths::recycle_bin_dir(root).join(&id);
        let manifest_path = target.join("manifest.json");
        if manifest_path.is_file() {
            let manifest: EntryManifest = serde_json::from_slice(
                &fs::read(&manifest_path).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("invalid migrated backup manifest {id}: {error}"))?;
            let source_records = records_for_source(&source, Some(root))?;
            let stored_records =
                read_file_records_with_cancellation(&target.join("files"), Some(root))?;
            if manifest.schema_version != 1
                || manifest.id != id
                || source_records != manifest.files
                || stored_records != manifest.files
            {
                return Err(format!("migrated backup manifest does not match {id}"));
            }
            remove_legacy_source_with_cancellation(
                root,
                &source,
                kind.is_dir(),
                &name,
                &target.join("files"),
            )?;
            migrated += 1;
            continue;
        }
        if target.exists() {
            return Err(format!(
                "recycle-bin entry {id} already exists without a valid manifest"
            ));
        }
        fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        let files = target.join("files");
        let restore_mod_id = if prefix.starts_with("legacy-mod-backup-")
            || prefix.starts_with("legacy-removed-mod-")
        {
            legacy_mod_id(&source)
        } else {
            None
        };
        let restore_root_files = prefix.starts_with("legacy-update-backup-");
        let result = (|| {
            fs::create_dir(&files).map_err(|error| error.to_string())?;
            let records = if kind.is_dir() {
                copy_backup_tree_with_cancel(&source, &source, &files, Some(root))?
            } else {
                let name = source
                    .file_name()
                    .ok_or("legacy file has no name")?
                    .to_string_lossy()
                    .into_owned();
                vec![copy_verified_file_with_progress_and_cancel(
                    &source,
                    &files.join(&name),
                    &name,
                    Some(root),
                    |_, _| Ok(()),
                )?]
            };
            let source_records = records_for_source(&source, Some(root))?;
            if records != source_records {
                return Err(format!("legacy storage verification failed: {name}"));
            }
            write_manifest(
                root,
                &target,
                &id,
                reason,
                records,
                RestoreOptions {
                    mod_id: restore_mod_id,
                    root_files: restore_root_files,
                    schematic_name: None,
                    catalog_entry: None,
                    paths: None,
                },
            )?;
            remove_legacy_source_with_cancellation(root, &source, kind.is_dir(), &name, &files)?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&target);
            return Err(error);
        }
        migrated += 1;
    }
    Ok(migrated)
}

fn legacy_mod_id(source: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(source).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return None;
    }
    let name = source.file_name()?.to_str()?;
    let (id, timestamp) = name.rsplit_once('-')?;
    if !timestamp.bytes().all(|byte| byte.is_ascii_digit())
        || !shroudforge_package::package_lookup::valid_identifier(id)
    {
        return None;
    }
    let manifest_path = source.join("mod.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).ok()?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return None;
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).ok()?).ok()?;
    (manifest["id"].as_str() == Some(id)).then(|| id.to_owned())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecycleBinEntry {
    id: String,
    reason: String,
    created_at: u64,
    expires_at: u64,
    file_count: usize,
    size_bytes: u64,
    can_restore: bool,
    restore_mod_id: Option<String>,
}

pub(crate) fn list_entries(root: &Path) -> Result<Vec<serde_json::Value>, String> {
    let directory = shroudforge_loader_config::paths::recycle_bin_dir(root);
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let retention = retention_seconds(root);
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let Ok(manifest) = read_manifest(&entry.path()) else {
            continue;
        };
        entries.push(
            serde_json::to_value(RecycleBinEntry {
                id: manifest.id,
                reason: manifest.reason,
                created_at: manifest.created_at,
                expires_at: manifest.created_at.saturating_add(retention),
                file_count: manifest.files.len(),
                size_bytes: manifest
                    .files
                    .iter()
                    .fold(0u64, |total, file| total.saturating_add(file.size)),
                can_restore: manifest.restore_mod_id.is_some()
                    || manifest.restore_root_files
                    || manifest.restore_schematic_name.is_some(),
                restore_mod_id: manifest.restore_mod_id,
            })
            .map_err(|error| error.to_string())?,
        );
    }
    entries.sort_by(|left, right| right["createdAt"].as_u64().cmp(&left["createdAt"].as_u64()));
    Ok(entries)
}

pub(crate) fn recycle_mod(root: &Path, mod_id: &str) -> Result<(), String> {
    let _operation_lock = acquire_operation_lock(root)?;
    ensure_queue_idle(root, "mods cannot be removed while an update is running")?;
    if !shroudforge_package::package_lookup::valid_identifier(mod_id) {
        return Err("invalid mod id".into());
    }
    let source = shroudforge_package::package_lookup::find_mod_package(root, mod_id)
        .ok_or("mod package not found")?;
    recycle_mod_directory(root, &source, mod_id, "Removed installed mod")
}

pub(crate) fn recycle_mod_directory(
    root: &Path,
    source: &Path,
    mod_id: &str,
    reason: &str,
) -> Result<(), String> {
    if !shroudforge_package::package_lookup::valid_identifier(mod_id) {
        return Err("invalid mod id".into());
    }
    let source_metadata = fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        return Err(format!(
            "installed mod package is not a regular directory: {}",
            source.display()
        ));
    }
    let manifest_path = source.join("mod.json");
    let manifest_metadata =
        fs::symlink_metadata(&manifest_path).map_err(|error| error.to_string())?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err(format!(
            "installed mod manifest is not a regular file: {}",
            manifest_path.display()
        ));
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| error.to_string())?)
            .map_err(|error| format!("invalid installed mod manifest: {error}"))?;
    if manifest["id"].as_str() != Some(mod_id) {
        return Err(format!(
            "installed mod manifest ID does not match {mod_id}: {}",
            source.display()
        ));
    }
    let package = shroudforge_package::config::read_manifest_path(root, source)
        .map_err(|error| format!("could not inspect installed mod package: {error}"))?;
    let live_runtime = runtime_is_active(root)
        && package
            .capabilities
            .iter()
            .any(|capability| capability.requires_runtime());
    if live_runtime {
        shroudforge_updater::request_runtime_mod_unload(root, &[mod_id.to_owned()]).map_err(
            |error| format!("could not safely unload '{mod_id}' before removing it: {error}"),
        )?;
    }
    let id = format!(
        "removed-mod-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let previous_catalog = catalog_entry_for_mod(root, mod_id);
    let saved = match preserve_mod_directory_with_cancellation(
        root,
        source,
        &id,
        reason,
        mod_id,
        previous_catalog.clone(),
        Some(root),
    ) {
        Ok(saved) => saved,
        Err(error) => {
            return Err(reload_runtime_after_remove_prepare(
                root,
                mod_id,
                live_runtime,
                error,
            ));
        }
    };
    if let Err(error) = crate::check_update_cancelled(root) {
        let cleanup = saved
            .parent()
            .ok_or_else(|| "recycle-bin entry has no parent".to_owned())
            .and_then(|entry| fs::remove_dir_all(entry).map_err(|error| error.to_string()));
        let error = match cleanup {
            Ok(()) => error,
            Err(cleanup_error) => {
                format!("{error}. Could not remove the prepared recycle-bin copy: {cleanup_error}")
            }
        };
        return Err(reload_runtime_after_remove_prepare(
            root,
            mod_id,
            live_runtime,
            error,
        ));
    }
    if let Err(error) = remove_directory_with_cancellation(root, source) {
        let rollback = restore_removed_mod(
            root,
            source,
            &saved,
            mod_id,
            previous_catalog.as_ref(),
            live_runtime,
        );
        return Err(match rollback {
            Ok(()) if error == "UPDATE_CANCELLED" => error,
            Ok(()) => format!(
                "could not remove mod package: {error}. Save your work, close Enshrouded, ShroudEdit, or any editor window using this package, then retry."
            ),
            Err(rollback_error) => format!(
                "could not remove mod package: {error}. Restoring the package also failed: {rollback_error}"
            ),
        });
    }
    if let Err(error) = crate::mod_installation::catalog::forget_catalog_install(root, mod_id) {
        let rollback = restore_removed_mod(
            root,
            source,
            &saved,
            mod_id,
            previous_catalog.as_ref(),
            live_runtime,
        );
        return Err(match rollback {
            Ok(()) => format!("could not update mod catalog state: {error}"),
            Err(rollback_error) => format!(
                "could not update mod catalog state: {error}. Package rollback also failed: {rollback_error}"
            ),
        });
    }
    Ok(())
}

fn reload_runtime_after_remove_prepare(
    root: &Path,
    mod_id: &str,
    reload_runtime: bool,
    error: String,
) -> String {
    if !reload_runtime {
        return error;
    }
    match shroudforge_updater::request_runtime_mod_reload(root, &[mod_id.to_owned()]) {
        Ok(()) => error,
        Err(reload_error) => format!(
            "{error}. The installed mod remains in place, but its runtime recovery also failed: {reload_error}"
        ),
    }
}

fn restore_removed_mod(
    root: &Path,
    source: &Path,
    saved: &Path,
    mod_id: &str,
    previous_catalog: Option<&(String, String)>,
    reload_runtime: bool,
) -> Result<(), String> {
    if source.exists() {
        fs::remove_dir_all(source)
            .map_err(|error| format!("could not clear partial mod package: {error}"))?;
    }
    restore_directory(saved, source)?;
    match previous_catalog {
        Some((project, version)) => crate::mod_installation::catalog::remember_catalog_install(
            root, project, version, mod_id, source,
        )?,
        None => crate::mod_installation::catalog::forget_catalog_install(root, mod_id)?,
    }
    if reload_runtime {
        shroudforge_updater::request_runtime_mod_reload(root, &[mod_id.to_owned()])
            .map_err(|error| format!("mod was restored but runtime recovery failed: {error}"))?;
    }
    fs::remove_dir_all(saved.parent().ok_or("recycle-bin entry has no parent")?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn catalog_entry_for_mod(root: &Path, mod_id: &str) -> Option<(String, String)> {
    crate::mod_installation::catalog::read_catalog_install_registry(root)
        .get("projects")
        .and_then(serde_json::Value::as_object)
        .and_then(|projects| {
            projects.iter().find_map(|(project_id, value)| {
                (value.get("modId").and_then(serde_json::Value::as_str) == Some(mod_id))
                    .then(|| {
                        value
                            .get("versionId")
                            .and_then(serde_json::Value::as_str)
                            .map(|version_id| (project_id.clone(), version_id.to_owned()))
                    })
                    .flatten()
            })
        })
}

pub(crate) fn restore_entry(root: &Path, id: &str) -> Result<(), String> {
    let _operation_lock = acquire_operation_lock(root)?;
    ensure_queue_idle(
        root,
        "recycle-bin entries cannot be restored while an update is running",
    )?;
    validate_entry_id(id)?;
    let entry = shroudforge_loader_config::paths::recycle_bin_dir(root).join(id);
    let manifest = read_manifest(&entry)?;
    let saved = entry.join("files");
    if read_file_records(&saved)? != manifest.files {
        return Err("recycle-bin entry failed its integrity check".into());
    }
    if let Some(mod_id) = manifest.restore_mod_id.as_deref() {
        restore_mod_entry(root, &entry, &saved, &manifest, mod_id)
    } else if manifest.restore_root_files {
        restore_root_files_entry(root, &entry, &saved, &manifest)
    } else if let Some(name) = manifest.restore_schematic_name.as_deref() {
        restore_schematic_entry(root, &entry, &saved, &manifest, name)
    } else {
        Err("this recycle-bin entry cannot be restored".into())
    }
}

fn restore_mod_entry(
    root: &Path,
    entry: &Path,
    saved: &Path,
    manifest: &EntryManifest,
    mod_id: &str,
) -> Result<(), String> {
    if !shroudforge_package::package_lookup::valid_identifier(mod_id) {
        return Err("recycle-bin entry contains an invalid mod id".into());
    }
    let destination = shroudforge_loader_config::paths::mods_dir(root).join(mod_id);
    let restored_manifest = shroudforge_package::config::read_manifest_path(root, saved)
        .map_err(|error| format!("could not inspect the saved mod package: {error}"))?;
    if restored_manifest.id != mod_id {
        return Err("saved mod package identity does not match the recycle-bin entry".into());
    }
    let runtime_active = runtime_is_active(root);
    let mod_ids = vec![mod_id.to_owned()];
    if runtime_active {
        shroudforge_updater::request_runtime_mod_unload(root, &mod_ids).map_err(|error| {
            format!("could not safely unload '{mod_id}' before restoring its package: {error}")
        })?;
    }
    let previous_catalog = catalog_entry_for_mod(root, mod_id);
    let rollback = if destination.exists() {
        let id = format!("restore-mod-{}", unique_stamp());
        match preserve_mod_directory(
            root,
            &destination,
            &id,
            "Mod package replaced by recycle-bin restore",
            mod_id,
            previous_catalog.clone(),
        ) {
            Ok(rollback) => Some(rollback),
            Err(error) => {
                return Err(reload_runtime_after_failed_prepare(
                    root,
                    &mod_ids,
                    runtime_active,
                    error,
                ));
            }
        }
    } else {
        None
    };
    if destination.exists()
        && let Err(error) = fs::remove_dir_all(&destination)
    {
        return Err(rollback_mod_restore(
            root,
            &destination,
            rollback.as_deref(),
            previous_catalog.as_ref(),
            mod_id,
            runtime_active,
            format!("could not prepare mod restore: {error}"),
        ));
    }
    if let Err(error) = restore_directory(saved, &destination) {
        return Err(rollback_mod_restore(
            root,
            &destination,
            rollback.as_deref(),
            previous_catalog.as_ref(),
            mod_id,
            runtime_active,
            error,
        ));
    }
    let catalog_result = match (
        manifest.catalog_project_id.as_deref(),
        manifest.catalog_version_id.as_deref(),
    ) {
        (Some(project_id), Some(version_id)) => {
            crate::mod_installation::catalog::remember_catalog_install(
                root,
                project_id,
                version_id,
                mod_id,
                &destination,
            )
        }
        _ => crate::mod_installation::catalog::forget_catalog_install(root, mod_id),
    };
    if let Err(error) = catalog_result {
        return Err(rollback_mod_restore(
            root,
            &destination,
            rollback.as_deref(),
            previous_catalog.as_ref(),
            mod_id,
            runtime_active,
            format!("could not restore mod catalog state: {error}"),
        ));
    }
    if runtime_active
        && let Err(error) = shroudforge_updater::request_runtime_mod_reload(root, &mod_ids)
    {
        return Err(rollback_mod_restore(
            root,
            &destination,
            rollback.as_deref(),
            previous_catalog.as_ref(),
            mod_id,
            runtime_active,
            format!("restored runtime mod failed to load: {error}"),
        ));
    }
    fs::remove_dir_all(entry)
        .map_err(|error| format!("mod was restored but the recycle-bin entry remains: {error}"))
}

fn reload_runtime_after_failed_prepare(
    root: &Path,
    mod_ids: &[String],
    reload_runtime: bool,
    error: String,
) -> String {
    if !reload_runtime {
        return error;
    }
    match shroudforge_updater::request_runtime_mod_reload(root, mod_ids) {
        Ok(()) => error,
        Err(reload_error) => format!(
            "{error}. The installed package is unchanged, but its runtime recovery also failed: {reload_error}"
        ),
    }
}

fn rollback_mod_restore(
    root: &Path,
    destination: &Path,
    rollback: Option<&Path>,
    previous_catalog: Option<&(String, String)>,
    mod_id: &str,
    reload_runtime: bool,
    error: String,
) -> String {
    let result = (|| {
        if reload_runtime {
            shroudforge_updater::request_runtime_mod_unload(root, &[mod_id.to_owned()])
                .map_err(|error| format!("could not unload the attempted package: {error}"))?;
        }
        if destination.exists() {
            fs::remove_dir_all(destination).map_err(|error| error.to_string())?;
        }
        if let Some(saved) = rollback {
            restore_directory(saved, destination)?;
        }
        match previous_catalog {
            Some((project, version)) => {
                crate::mod_installation::catalog::remember_catalog_install(
                    root,
                    project,
                    version,
                    mod_id,
                    destination,
                )?;
            }
            None => crate::mod_installation::catalog::forget_catalog_install(root, mod_id)?,
        }
        if reload_runtime {
            shroudforge_updater::request_runtime_mod_reload(root, &[mod_id.to_owned()]).map_err(
                |error| {
                    format!("previous package was restored but runtime recovery failed: {error}")
                },
            )?;
        }
        if let Some(saved) = rollback {
            fs::remove_dir_all(saved.parent().ok_or("rollback entry has no parent")?)
                .map_err(|error| error.to_string())?;
        }
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) if rollback.is_some() => {
            format!("restore failed and the previous mod was recovered: {error}")
        }
        Ok(()) => format!("restore failed and no previous mod package was available: {error}"),
        Err(rollback_error) => format!(
            "restore failed: {error}. Restoring the previous mod also failed: {rollback_error}"
        ),
    }
}

fn restore_root_files_entry(
    root: &Path,
    entry: &Path,
    saved: &Path,
    manifest: &EntryManifest,
) -> Result<(), String> {
    let paths = if manifest.restore_paths.is_empty() {
        manifest
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>()
    } else {
        manifest.restore_paths.clone()
    };
    if paths.is_empty() {
        return Err("this recycle-bin entry contains no files to restore".into());
    }
    for path in &paths {
        validate_release_path(path)?;
        crate::validate_update_paths::validate_file_path(root, path)?;
    }
    #[cfg(windows)]
    crate::scheduled::prepare_release_file_locks(root, &paths)?;
    let rollback_id = format!("restore-files-{}", unique_stamp());
    let rollback_files = preserve_update_files(
        root,
        &rollback_id,
        "Files replaced by recycle-bin restore",
        &paths,
    )?;
    let rollback_manifest = read_manifest(
        rollback_files
            .parent()
            .ok_or("rollback entry has no parent")?,
    )?;
    if let Err(error) = apply_root_files(root, saved, &manifest.files, &paths) {
        let rollback_result =
            apply_root_files(root, &rollback_files, &rollback_manifest.files, &paths);
        return match rollback_result {
            Ok(()) => {
                let _ = rollback_files.parent().map(fs::remove_dir_all);
                Err(format!(
                    "restore failed and the previous files were recovered: {error}"
                ))
            }
            Err(rollback_error) => Err(format!(
                "restore failed: {error}. Restoring the previous files also failed: {rollback_error}"
            )),
        };
    }
    fs::remove_dir_all(entry)
        .map_err(|error| format!("files were restored but the recycle-bin entry remains: {error}"))
}

fn apply_root_files(
    root: &Path,
    saved: &Path,
    files: &[StoredFile],
    paths: &[String],
) -> Result<(), String> {
    let existing = files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<std::collections::HashSet<_>>();
    for relative in paths {
        validate_release_path(relative)?;
        crate::validate_update_paths::validate_file_path(root, relative)?;
        let destination = installed_path(root, relative);
        if !existing.contains(relative.as_str()) && destination.exists() {
            fs::remove_file(&destination)
                .map_err(|error| format!("could not remove restored target {relative}: {error}"))?;
        }
    }
    for file in files {
        let relative = validate_release_path(&file.path)?;
        let source = saved.join(relative);
        let destination = installed_path(root, &file.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let copied = copy_verified_file(&source, &destination, &file.path)?;
        if copied != *file {
            return Err(format!("restored file failed verification: {}", file.path));
        }
    }
    Ok(())
}

fn restore_schematic_entry(
    root: &Path,
    entry: &Path,
    saved: &Path,
    manifest: &EntryManifest,
    original_name: &str,
) -> Result<(), String> {
    if manifest.files.len() != 1 || manifest.files[0].path != format!("{original_name}.sfbp") {
        return Err("recycle-bin schematic entry failed its integrity check".into());
    }
    let saved_file = saved.join(format!("{original_name}.sfbp"));
    let metadata = fs::symlink_metadata(&saved_file).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("recycle-bin schematic is not a regular file".into());
    }
    if metadata.len() > shroudforge_schematics::validate_schematic_v8::MAX_BYTES as u64 {
        return Err("recycle-bin schematic exceeds the 256 MiB size limit".into());
    }
    let bytes = fs::read(&saved_file).map_err(|error| error.to_string())?;
    shroudforge_schematics::schematic_library::restore_recycled_schematic(
        root,
        original_name,
        &bytes,
    )?;
    fs::remove_dir_all(entry)
        .map_err(|error| format!("schematic restored but recycle-bin entry remains: {error}"))
}

fn read_manifest(entry: &Path) -> Result<EntryManifest, String> {
    let metadata = fs::symlink_metadata(entry).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("recycle-bin entry is not a regular directory".into());
    }
    let manifest: EntryManifest = serde_json::from_slice(
        &fs::read(entry.join("manifest.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if manifest.schema_version != 1
        || entry.file_name().and_then(|name| name.to_str()) != Some(manifest.id.as_str())
        || validate_entry_id(&manifest.id).is_err()
        || manifest
            .restore_schematic_name
            .as_deref()
            .is_some_and(|name| {
                !shroudforge_schematics::schematic_library::valid_name(name)
                    || manifest.restore_mod_id.is_some()
                    || manifest.restore_root_files
                    || !manifest.restore_paths.is_empty()
            })
    {
        return Err("recycle-bin manifest does not match its entry".into());
    }
    Ok(manifest)
}

fn unique_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

pub(crate) fn empty(root: &Path) -> Result<usize, String> {
    let _operation_lock = acquire_operation_lock(root)?;
    ensure_queue_idle(
        root,
        "the recycle bin cannot be emptied while an update is running",
    )?;
    let entries = list_entries(root)?;
    let directory = shroudforge_loader_config::paths::recycle_bin_dir(root);
    for entry in &entries {
        let id = entry["id"].as_str().ok_or("recycle-bin entry has no ID")?;
        fs::remove_dir_all(directory.join(id))
            .map_err(|error| format!("could not remove recycle-bin entry {id}: {error}"))?;
    }
    Ok(entries.len())
}

pub(crate) fn delete_entry(root: &Path, id: &str) -> Result<(), String> {
    let _operation_lock = acquire_operation_lock(root)?;
    ensure_queue_idle(
        root,
        "recycle-bin entries cannot be deleted while an update is running",
    )?;
    delete_entry_inner(root, id)
}

fn delete_entry_inner(root: &Path, id: &str) -> Result<(), String> {
    validate_entry_id(id)?;
    let entry = shroudforge_loader_config::paths::recycle_bin_dir(root).join(id);
    let _ = read_manifest(&entry)?;
    fs::remove_dir_all(&entry)
        .map_err(|error| format!("could not delete recycle-bin entry {id}: {error}"))
}

fn ensure_queue_idle(root: &Path, message: &str) -> Result<(), String> {
    let queue = crate::update_queue_storage::read_update_queue_value(root);
    if let Some(error) = queue["error"].as_str() {
        return Err(error.to_owned());
    }
    if queue["run"]["state"] == "running" {
        return Err(message.into());
    }
    Ok(())
}

fn remove_legacy_source_with_cancellation(
    root: &Path,
    source: &Path,
    is_directory: bool,
    name: &str,
    backup_files: &Path,
) -> Result<(), String> {
    let mut check_cancelled = || crate::check_update_cancelled(root);
    remove_legacy_source_with_check(
        source,
        is_directory,
        name,
        backup_files,
        &mut check_cancelled,
    )
}

fn remove_legacy_source_with_check(
    source: &Path,
    is_directory: bool,
    name: &str,
    backup_files: &Path,
    check_cancelled: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    check_cancelled()?;
    let removal = if is_directory {
        remove_directory_with_check(source, check_cancelled)
    } else {
        fs::remove_file(source).map_err(|error| error.to_string())
    };
    if let Err(error) = removal {
        if is_directory && error == "UPDATE_CANCELLED" {
            if let Err(restore_error) = restore_legacy_directory(source, backup_files) {
                return Err(format!(
                    "UPDATE_CANCELLED. Legacy folder recovery failed for {}: {restore_error}",
                    source.display()
                ));
            }
            return Err(error);
        }
        return Err(format!(
            "migrated legacy entry {name} remains in its old folder: {error}"
        ));
    }
    Ok(())
}

fn remove_directory_with_cancellation(root: &Path, directory: &Path) -> Result<(), String> {
    let mut check_cancelled = || crate::check_update_cancelled(root);
    remove_directory_with_check(directory, &mut check_cancelled)
}

fn remove_directory_with_check(
    directory: &Path,
    check_cancelled: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    check_cancelled()?;
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        check_cancelled()?;
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if is_reparse_point(&metadata) {
            if metadata.is_dir() {
                fs::remove_dir(&path).map_err(|error| error.to_string())?;
            } else {
                fs::remove_file(&path).map_err(|error| error.to_string())?;
            }
        } else if metadata.is_dir() {
            remove_directory_with_check(&path, check_cancelled)?;
        } else {
            fs::remove_file(&path).map_err(|error| error.to_string())?;
        }
    }
    check_cancelled()?;
    fs::remove_dir(directory).map_err(|error| error.to_string())
}

fn restore_legacy_directory(source: &Path, backup_files: &Path) -> Result<(), String> {
    if source.exists() {
        fs::remove_dir_all(source).map_err(|error| {
            format!("could not clear the partially removed legacy folder: {error}")
        })?;
    }
    fs::create_dir_all(source).map_err(|error| error.to_string())?;
    copy_backup_tree_with_cancel(backup_files, backup_files, source, None)?;
    if read_file_records(source)? != read_file_records(backup_files)? {
        return Err("restored legacy folder does not match its verified backup".into());
    }
    Ok(())
}

fn records_for_source(
    source: &Path,
    cancel_root: Option<&Path>,
) -> Result<Vec<StoredFile>, String> {
    if source.is_dir() {
        read_file_records_with_cancellation(source, cancel_root)
    } else {
        Ok(vec![stored_file_with_cancellation(
            source,
            &source
                .file_name()
                .ok_or("legacy file has no name")?
                .to_string_lossy(),
            cancel_root,
        )?])
    }
}

fn write_manifest(
    root: &Path,
    entry: &Path,
    id: &str,
    reason: &str,
    files: Vec<StoredFile>,
    restore: RestoreOptions,
) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let restore_paths = restore.paths.unwrap_or_else(|| {
        if restore.root_files {
            files.iter().map(|file| file.path.clone()).collect()
        } else {
            Vec::new()
        }
    });
    let manifest = EntryManifest {
        schema_version: 1,
        id: id.to_owned(),
        reason: reason.to_owned(),
        created_at: now,
        expires_at: now.saturating_add(retention_seconds(root)),
        files,
        restore_paths,
        restore_mod_id: restore.mod_id,
        restore_root_files: restore.root_files,
        restore_schematic_name: restore.schematic_name,
        catalog_project_id: restore.catalog_entry.as_ref().map(|entry| entry.0.clone()),
        catalog_version_id: restore.catalog_entry.map(|entry| entry.1),
    };
    let manifest = serde_json::to_value(manifest).map_err(|error| error.to_string())?;
    shroudforge_loader_config::config::write_json(&entry.join("manifest.json"), &manifest)
        .map_err(|error| error.to_string())
}

pub(crate) fn purge_expired(root: &Path) -> Result<usize, String> {
    let directory = shroudforge_loader_config::paths::recycle_bin_dir(root);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut removed = 0;
    for entry in list_entries(root)? {
        crate::check_update_cancelled(root)?;
        if entry["expiresAt"].as_u64().unwrap_or(u64::MAX) > now {
            continue;
        }
        let id = entry["id"].as_str().ok_or("recycle-bin entry has no ID")?;
        fs::remove_dir_all(directory.join(id))
            .map_err(|error| format!("could not remove expired recycle-bin entry {id}: {error}"))?;
        removed += 1;
        crate::check_update_cancelled(root)?;
        log(
            root,
            'I',
            &format!("Removed expired recycle-bin entry {id}"),
        )?;
    }
    Ok(removed)
}

fn validate_entry_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 120
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err("recycle-bin entry has an invalid ID".into());
    }
    Ok(())
}

fn copy_backup_tree_with_cancel(
    source_root: &Path,
    source: &Path,
    target_root: &Path,
    cancel_root: Option<&Path>,
) -> Result<Vec<StoredFile>, String> {
    if let Some(root) = cancel_root {
        crate::check_update_cancelled(root)?;
    }
    let mut records = Vec::new();
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        if let Some(root) = cancel_root {
            crate::check_update_cancelled(root)?;
        }
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let relative = path
            .strip_prefix(source_root)
            .map_err(|error| error.to_string())?;
        let destination = target_root.join(relative);
        if kind.is_symlink() {
            return Err(format!(
                "legacy backup contains a symbolic link: {}",
                path.display()
            ));
        }
        if kind.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
            records.extend(copy_backup_tree_with_cancel(
                source_root,
                &path,
                target_root,
                cancel_root,
            )?);
        } else if kind.is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            copy_verified_file_with_progress_and_cancel(
                &path,
                &destination,
                &relative.to_string_lossy().replace('\\', "/"),
                cancel_root,
                |_, _| Ok(()),
            )?;
            records.push(stored_file_with_cancellation(
                &destination,
                &relative.to_string_lossy().replace('\\', "/"),
                cancel_root,
            )?);
        }
    }
    records.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(records)
}

fn copy_verified_file(
    source: &Path,
    destination: &Path,
    relative: &str,
) -> Result<StoredFile, String> {
    copy_verified_file_with_progress_and_cancel(source, destination, relative, None, |_, _| Ok(()))
}

fn copy_verified_file_with_progress_and_cancel(
    source: &Path,
    destination: &Path,
    relative: &str,
    cancel_root: Option<&Path>,
    progress: impl FnMut(u64, u64) -> Result<(), String>,
) -> Result<StoredFile, String> {
    crate::apply_release_files::copy_file_with_progress_and_cancel(
        source,
        destination,
        cancel_root,
        progress,
    )
    .map_err(|error| {
        if error == "UPDATE_CANCELLED" {
            error
        } else {
            format!("could not preserve {relative} in the recycle bin: {error}")
        }
    })?;
    let source_record = stored_file_with_cancellation(source, relative, cancel_root)?;
    let copied_record = stored_file_with_cancellation(destination, relative, cancel_root)?;
    if source_record != copied_record {
        return Err(format!("recycle-bin copy verification failed: {relative}"));
    }
    Ok(source_record)
}

fn read_file_records(root: &Path) -> Result<Vec<StoredFile>, String> {
    read_file_records_with_cancellation(root, None)
}

fn read_file_records_with_cancellation(
    root: &Path,
    cancel_root: Option<&Path>,
) -> Result<Vec<StoredFile>, String> {
    let mut records = Vec::new();
    collect_file_records(root, root, &mut records, cancel_root)?;
    records.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(records)
}

fn collect_file_records(
    root: &Path,
    directory: &Path,
    records: &mut Vec<StoredFile>,
    cancel_root: Option<&Path>,
) -> Result<(), String> {
    if let Some(root) = cancel_root {
        crate::check_update_cancelled(root)?;
    }
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        if let Some(root) = cancel_root {
            crate::check_update_cancelled(root)?;
        }
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_symlink() {
            return Err(format!(
                "recycle-bin entry contains a symbolic link: {}",
                path.display()
            ));
        }
        if kind.is_dir() {
            collect_file_records(root, &path, records, cancel_root)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            records.push(stored_file_with_cancellation(
                &path,
                &relative,
                cancel_root,
            )?);
        }
    }
    Ok(())
}

fn stored_file_with_cancellation(
    path: &Path,
    relative: &str,
    cancel_root: Option<&Path>,
) -> Result<StoredFile, String> {
    use sha2::Digest;
    use std::io::Read;

    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if let Some(root) = cancel_root {
            crate::check_update_cancelled(root)?;
        }
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(StoredFile {
        path: relative.replace('\\', "/"),
        size,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

fn log(root: &Path, level: char, message: &str) -> Result<(), String> {
    shroudforge_logging::append(root, level, "updater", message)
        .map(|_| ())
        .map_err(|error| format!("could not write ShroudForge log: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const V7_PROPS: &[u8] = b"SHROUDFORGE_WORLD_BLUEPRINT_V7\ny\n1,1,1\nprops\n-\n-\n-\n-\n-\n1\n42,0.5,0.5,0.5,0,0,0,1,1,1,1,FEDCBA9876543210,0123456789ABCDEF";

    fn create_v7_schematic(root: &Path) -> PathBuf {
        let directory = shroudforge_schematics::schematic_library::directory(root);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("old_building.sfbp");
        fs::write(&path, V7_PROPS).unwrap();
        path
    }

    #[test]
    fn schematic_conversion_moves_v7_to_recycle_bin_and_restores_it_separately() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let original = create_v7_schematic(root);

        convert_schematic_to_latest(root, "old_building").unwrap();

        let converted = fs::read(&original).unwrap();
        assert!(shroudforge_schematics::validate_schematic_v8::validate(&converted).is_ok());
        let entries = list_entries(root).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["canRestore"], true);
        let id = entries[0]["id"].as_str().unwrap();
        let saved = shroudforge_loader_config::paths::recycle_bin_dir(root)
            .join(id)
            .join("files/old_building.sfbp");
        assert_eq!(fs::read(saved).unwrap(), V7_PROPS);

        restore_entry(root, id).unwrap();

        assert_eq!(fs::read(&original).unwrap(), converted);
        let restored =
            shroudforge_schematics::schematic_library::directory(root).join("old_building_v7.sfbp");
        assert_eq!(fs::read(restored).unwrap(), V7_PROPS);
        assert!(list_entries(root).unwrap().is_empty());
    }

    #[test]
    fn automatic_schematic_conversion_uses_the_same_recycle_bin_entry() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let original = create_v7_schematic(root);
        shroudforge_loader_config::config::update_loader(root, |config| {
            config["modules"]["schematics"]["conversionMode"] = "automatic".into();
            Ok(())
        })
        .unwrap();

        convert_automatic_schematics(root).unwrap();
        convert_automatic_schematics(root).unwrap();

        assert!(shroudforge_schematics::schematic_library::list(root).unwrap()[0].valid);
        assert!(
            shroudforge_schematics::validate_schematic_v8::validate(&fs::read(original).unwrap())
                .is_ok()
        );
        assert_eq!(list_entries(root).unwrap().len(), 1);
    }

    #[test]
    fn cancellation_during_legacy_folder_removal_restores_the_source() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("legacy");
        let backup = temporary.path().join("recycle-entry").join("files");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&backup).unwrap();
        fs::write(source.join("one.bin"), b"first").unwrap();
        fs::write(source.join("two.bin"), b"second").unwrap();
        fs::write(backup.join("one.bin"), b"first").unwrap();
        fs::write(backup.join("two.bin"), b"second").unwrap();

        let mut checks = 0;
        let mut check_cancelled = || {
            checks += 1;
            if checks == 4 {
                Err("UPDATE_CANCELLED".to_owned())
            } else {
                Ok(())
            }
        };
        let result =
            remove_legacy_source_with_check(&source, true, "legacy", &backup, &mut check_cancelled);

        assert_eq!(result.unwrap_err(), "UPDATE_CANCELLED");
        assert_eq!(fs::read(source.join("one.bin")).unwrap(), b"first");
        assert_eq!(fs::read(source.join("two.bin")).unwrap(), b"second");
        assert!(read_file_records(&source).unwrap() == read_file_records(&backup).unwrap());
    }
}
