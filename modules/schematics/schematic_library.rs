use base64::Engine as _;
use image::{DynamicImage, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_SCHEMATIC_BYTES: u64 = crate::validate_schematic_v8::MAX_BYTES as u64;
const MAX_COVER_BYTES: usize = 64 * 1024 * 1024;
const MAX_COVER_DIMENSION: u32 = 8192;
const MAX_COVER_PIXELS: u64 = 32 * 1024 * 1024;
const ASSOCIATED_SUFFIXES: &[&str] = &[
    ".sfbp",
    ".png",
    ".thumb.png",
    ".metadata.json",
    ".cover-undo.json",
    ".cover-undo.png",
    ".cover-undo.thumb.png",
];
const MIGRATION_SUFFIXES: &[&str] = &[
    ".metadata.json",
    ".cover-undo.json",
    ".cover-undo.png",
    ".cover-undo.thumb.png",
    ".png",
    ".thumb.png",
    ".sfbp",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicInfo {
    pub name: String,
    pub image: Option<String>,
    pub source: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub valid: bool,
    pub format_version: Option<u32>,
    pub convertible: bool,
    pub validation_error: Option<String>,
    pub size: u64,
    pub modified_at: u64,
}

pub struct CatalogSchematic {
    pub name: String,
    pub project_id: String,
    pub version_id: String,
    pub modified: bool,
}

static VALIDATION_CACHE: LazyLock<
    Mutex<
        HashMap<
            PathBuf,
            (
                u64,
                Option<SystemTime>,
                crate::convert_schematic_v7::Inspection,
            ),
        >,
    >,
> = LazyLock::new(|| Mutex::new(HashMap::new()));
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchematicMetadata {
    schema_version: u32,
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duplicate_of: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoverUndo {
    had_cover: bool,
    had_thumbnail: bool,
}

pub fn directory(root: &Path) -> PathBuf {
    shroudforge_loader_config::paths::export_dir(root).join("schematics")
}

pub fn ensure_library_directory(root: &Path) -> Result<PathBuf, String> {
    ensure_directory(root)
}

pub fn list(root: &Path) -> Result<Vec<SchematicInfo>, String> {
    crate::require_enabled(root)?;
    let directory = directory(root);
    let Some(_) = regular_directory_if_present(&directory)? else {
        return Ok(Vec::new());
    };
    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("Could not read the Schematics folder: {error}"))?;
    let mut schematics = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not read a Schematics entry: {error}"))?;
        let path = entry.path();
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("sfbp"))
        {
            continue;
        }
        let file_metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Could not inspect schematic {}: {error}", path.display()))?;
        if file_metadata.file_type().is_symlink() || !file_metadata.is_file() {
            return Err(format!(
                "Schematic entry is not a regular file: {}",
                path.display()
            ));
        }
        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if !valid_name(name) {
            continue;
        }
        let inspection = cached_inspection(&path, &file_metadata);
        let metadata_record = read_metadata(&directory, name);
        let image = read_preview(&directory, name);
        let modified_at = file_metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |value| value.as_secs());
        schematics.push(SchematicInfo {
            name: name.to_owned(),
            image,
            source: metadata_record
                .as_ref()
                .map(|value| value.source.as_str())
                .filter(|value| *value == "shroudedit")
                .unwrap_or("local")
                .to_owned(),
            project_id: metadata_record
                .as_ref()
                .and_then(|value| value.project_id.clone()),
            version_id: metadata_record
                .as_ref()
                .and_then(|value| value.version_id.clone()),
            valid: inspection.valid,
            format_version: inspection.format_version,
            convertible: inspection.convertible,
            validation_error: inspection.validation_error,
            size: file_metadata.len(),
            modified_at,
        });
    }
    schematics.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(schematics)
}

pub fn catalog_installations(root: &Path) -> Result<Vec<CatalogSchematic>, String> {
    let directory = directory(root);
    let mut entries = Vec::new();
    for item in list(root)? {
        if item.source != "shroudedit" {
            continue;
        }
        let Some(metadata) = read_metadata(&directory, &item.name) else {
            continue;
        };
        let (Some(project_id), Some(version_id)) = (metadata.project_id, metadata.version_id)
        else {
            continue;
        };
        let path = schematic_path(&directory, &item.name);
        regular_file_metadata(&path)?;
        let current_hash = hash_hex(file_hash(&path)?);
        let modified = metadata.content_sha256.as_deref() != Some(current_hash.as_str());
        entries.push(CatalogSchematic {
            name: item.name,
            project_id,
            version_id,
            modified,
        });
    }
    Ok(entries)
}

pub fn read(root: &Path, name: &str) -> Result<Vec<u8>, String> {
    crate::require_enabled(root)?;
    validate_name(name)?;
    let directory = directory(root);
    let path = schematic_path(&directory, name);
    let metadata = regular_file_metadata(&path)?;
    if metadata.len() > MAX_SCHEMATIC_BYTES {
        return Err("Schematic exceeds the 256 MiB size limit".into());
    }
    let bytes = fs::read(&path).map_err(|error| format!("Could not read schematic: {error}"))?;
    validate_schematic_bytes(&bytes)?;
    Ok(bytes)
}

pub fn save(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    crate::require_enabled(root)?;
    validate_name(name)?;
    validate_schematic_bytes(bytes)?;
    if bytes.len() as u64 > MAX_SCHEMATIC_BYTES {
        return Err("Schematic exceeds the 256 MiB size limit".into());
    }
    let directory = ensure_directory(root)?;
    let destination = schematic_path(&directory, name);
    if destination.exists() {
        return Err("A schematic with that name already exists".into());
    }
    write_new_file(&destination, bytes)?;
    clear_validation_cache(&destination);
    Ok(())
}

/// Imports a library file while leaving the public Lua save contract on the current format.
pub fn import_schematic(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    crate::require_enabled(root)?;
    validate_name(name)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_SCHEMATIC_BYTES {
        return Err("Schematic has an unsupported file size".into());
    }
    let inspection = crate::convert_schematic_v7::inspect(bytes);
    if !inspection.valid && !inspection.convertible {
        return Err(format!(
            "Schematic cannot be imported: {}",
            inspection
                .validation_error
                .unwrap_or_else(|| "unrecognized SFBP format".into())
        ));
    }
    let directory = ensure_directory(root)?;
    let destination = schematic_path(&directory, name);
    if destination.exists() {
        return Err("A schematic with that name already exists".into());
    }
    write_new_file(&destination, bytes)?;
    clear_validation_cache(&destination);
    Ok(())
}

pub fn restore_recycled_schematic(
    root: &Path,
    original_name: &str,
    bytes: &[u8],
) -> Result<String, String> {
    crate::require_enabled(root)?;
    validate_name(original_name)?;
    let inspection = crate::convert_schematic_v7::inspect(bytes);
    if !inspection.convertible || inspection.format_version != Some(7) {
        return Err("Recycle-bin entry is not a valid SFBP v7 schematic".into());
    }
    let directory = ensure_directory(root)?;
    let target_name = if path_exists(&schematic_path(&directory, original_name))? {
        let suffix = "_v7";
        let base = format!(
            "{}{}",
            &original_name[..original_name.len().min(64 - suffix.len())],
            suffix
        );
        unique_name(&directory, &base)
    } else {
        original_name.to_owned()
    };
    import_schematic(root, &target_name, bytes)?;
    Ok(target_name)
}

pub fn rename(root: &Path, old_name: &str, new_name: &str) -> Result<(), String> {
    crate::require_enabled(root)?;
    validate_name(old_name)?;
    validate_name(new_name)?;
    if old_name == new_name {
        return Err("Choose a different name for the schematic".into());
    }
    let directory = ensure_directory(root)?;
    let source = schematic_path(&directory, old_name);
    regular_file_metadata(&source)?;
    let destinations = associated_paths(&directory, new_name);
    for destination in &destinations {
        if destination.exists() {
            return Err("A schematic or related image already uses that name".into());
        }
    }
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for suffix in ASSOCIATED_SUFFIXES {
        let source = directory.join(format!("{old_name}{suffix}"));
        if !source.exists() {
            continue;
        }
        let destination = directory.join(format!("{new_name}{suffix}"));
        if let Err(error) = move_regular_file(&source, &destination) {
            for (from, to) in moved.into_iter().rev() {
                let _ = fs::rename(to, from);
            }
            return Err(error);
        }
        moved.push((source, destination));
    }
    clear_validation_cache(&schematic_path(&directory, old_name));
    clear_validation_cache(&schematic_path(&directory, new_name));
    Ok(())
}

pub fn duplicate(root: &Path, name: &str, new_name: &str) -> Result<(), String> {
    crate::require_enabled(root)?;
    validate_name(name)?;
    validate_name(new_name)?;
    if name == new_name {
        return Err("Choose a different name for the schematic copy".into());
    }
    let directory = ensure_directory(root)?;
    let source = schematic_path(&directory, name);
    regular_file_metadata(&source)?;
    let destination = schematic_path(&directory, new_name);
    if destination.exists() {
        return Err("A schematic with that name already exists".into());
    }
    let mut created = Vec::new();
    for suffix in [".sfbp", ".png", ".thumb.png"] {
        let source = directory.join(format!("{name}{suffix}"));
        if !source.exists() {
            continue;
        }
        let target = directory.join(format!("{new_name}{suffix}"));
        if let Err(error) = copy_new_file(&source, &target) {
            for path in created {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        created.push(target);
    }
    let metadata = SchematicMetadata {
        schema_version: 1,
        source: "local".into(),
        project_id: None,
        version_id: None,
        content_sha256: None,
        duplicate_of: Some(name.to_owned()),
    };
    if let Err(error) = write_metadata(&directory, new_name, &metadata) {
        for path in created {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    clear_validation_cache(&destination);
    Ok(())
}

pub fn delete(root: &Path, name: &str) -> Result<(), String> {
    crate::require_enabled(root)?;
    validate_name(name)?;
    let directory = ensure_directory(root)?;
    let source = schematic_path(&directory, name);
    regular_file_metadata(&source)?;
    let mut errors = Vec::new();
    for path in associated_paths(&directory, name) {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                errors.push(format!("{} is not a regular file", path.display()));
            }
            Ok(_) => {
                if let Err(error) = fs::remove_file(&path) {
                    errors.push(format!("could not remove {}: {error}", path.display()));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => errors.push(format!("could not inspect {}: {error}", path.display())),
        }
    }
    if errors.is_empty() {
        clear_validation_cache(&source);
        Ok(())
    } else {
        Err(errors.join(". "))
    }
}

pub fn set_cover(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    crate::require_enabled(root)?;
    validate_name(name)?;
    if bytes.is_empty() || bytes.len() > MAX_COVER_BYTES {
        return Err("Cover image must be smaller than 64 MiB".into());
    }
    let directory = ensure_directory(root)?;
    regular_file_metadata(&schematic_path(&directory, name))?;
    let (full, thumbnail) = normalize_cover(bytes)?;
    backup_cover(&directory, name)?;
    write_replace_file(&directory.join(format!("{name}.png")), &full)?;
    if let Err(error) = write_replace_file(&directory.join(format!("{name}.thumb.png")), &thumbnail)
    {
        let undo_path = directory.join(format!("{name}.cover-undo.json"));
        let rollback = fs::read(&undo_path)
            .map_err(|rollback_error| {
                format!("Could not read cover rollback data: {rollback_error}")
            })
            .and_then(|data| {
                serde_json::from_slice::<CoverUndo>(&data).map_err(|rollback_error| {
                    format!("Cover rollback data is invalid: {rollback_error}")
                })
            })
            .and_then(|undo| {
                restore_cover_file(
                    &directory,
                    &format!("{name}.png"),
                    &format!("{name}.cover-undo.png"),
                    undo.had_cover,
                )?;
                restore_cover_file(
                    &directory,
                    &format!("{name}.thumb.png"),
                    &format!("{name}.cover-undo.thumb.png"),
                    undo.had_thumbnail,
                )
            });
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "Could not update schematic cover: {error}. Rollback failed: {rollback_error}"
            )),
        };
    }
    Ok(())
}

pub fn undo_cover(root: &Path, name: &str) -> Result<(), String> {
    crate::require_enabled(root)?;
    validate_name(name)?;
    let directory = ensure_directory(root)?;
    regular_file_metadata(&schematic_path(&directory, name))?;
    let undo_path = directory.join(format!("{name}.cover-undo.json"));
    let undo: CoverUndo = serde_json::from_slice(
        &fs::read(&undo_path)
            .map_err(|error| format!("Could not read cover undo data: {error}"))?,
    )
    .map_err(|error| format!("Cover undo data is invalid: {error}"))?;
    restore_cover_file(
        &directory,
        &format!("{name}.png"),
        &format!("{name}.cover-undo.png"),
        undo.had_cover,
    )?;
    restore_cover_file(
        &directory,
        &format!("{name}.thumb.png"),
        &format!("{name}.cover-undo.thumb.png"),
        undo.had_thumbnail,
    )?;
    for suffix in [
        ".cover-undo.png",
        ".cover-undo.thumb.png",
        ".cover-undo.json",
    ] {
        let _ = fs::remove_file(directory.join(format!("{name}{suffix}")));
    }
    Ok(())
}

pub fn import_catalog_data(
    root: &Path,
    bytes: &[u8],
    project_id: &str,
    version_id: &str,
    title: &str,
) -> Result<String, String> {
    crate::require_enabled(root)?;
    validate_catalog_id(project_id)?;
    validate_catalog_id(version_id)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_SCHEMATIC_BYTES {
        return Err("Downloaded schematic has an unsupported file size".into());
    }
    validate_schematic_bytes(bytes)?;
    let directory = ensure_directory(root)?;
    let incoming_hash = hash_hex(Sha256::digest(bytes).into());
    if let Some(existing) = find_catalog_schematic(&directory, project_id)? {
        let destination = schematic_path(&directory, &existing);
        regular_file_metadata(&destination)?;
        let current_hash = hash_hex(file_hash(&destination)?);
        let previous =
            read_metadata(&directory, &existing).ok_or("Catalog schematic metadata is missing")?;
        if previous.version_id.as_deref() == Some(version_id) && current_hash == incoming_hash {
            if previous.content_sha256.is_none() {
                write_metadata(
                    &directory,
                    &existing,
                    &SchematicMetadata {
                        content_sha256: Some(current_hash),
                        ..previous
                    },
                )?;
            }
            return Ok(existing);
        }
        if previous.content_sha256.as_deref() != Some(current_hash.as_str()) {
            return Err("Catalog schematic was edited locally or has no verified original checksum. Its file was not replaced".into());
        }
        let schematic_backup = destination.with_file_name(format!(
            ".schematic-{}-{}.bak",
            std::process::id(),
            unique_stamp()
        ));
        copy_new_file(&destination, &schematic_backup)?;
        let metadata_path = directory.join(format!("{existing}.metadata.json"));
        let metadata_backup = if metadata_path.exists() {
            let metadata = regular_file_metadata(&metadata_path)?;
            if metadata.len() > 16 * 1024 {
                let _ = fs::remove_file(&schematic_backup);
                return Err("Schematic metadata is too large to update safely".into());
            }
            let backup = metadata_path.with_file_name(format!(
                ".schematic-{}-{}.bak",
                std::process::id(),
                unique_stamp()
            ));
            if let Err(error) = copy_new_file(&metadata_path, &backup) {
                let _ = fs::remove_file(&schematic_backup);
                return Err(error);
            }
            Some(backup)
        } else {
            None
        };
        let metadata = SchematicMetadata {
            schema_version: 1,
            source: "shroudedit".into(),
            project_id: Some(project_id.to_owned()),
            version_id: Some(version_id.to_owned()),
            content_sha256: Some(incoming_hash.clone()),
            duplicate_of: None,
        };
        let result = write_replace_file(&destination, bytes)
            .and_then(|()| write_metadata(&directory, &existing, &metadata));
        if let Err(error) = result {
            let restore_schematic = copy_replace_file(&schematic_backup, &destination);
            let restore_metadata = if let Some(backup) = metadata_backup.as_ref() {
                copy_replace_file(backup, &metadata_path)
            } else {
                match fs::remove_file(&metadata_path) {
                    Ok(()) => Ok(()),
                    Err(remove_error) if remove_error.kind() == std::io::ErrorKind::NotFound => {
                        Ok(())
                    }
                    Err(remove_error) => Err(format!(
                        "Could not restore schematic metadata: {remove_error}"
                    )),
                }
            };
            let _ = fs::remove_file(&schematic_backup);
            if let Some(backup) = metadata_backup {
                let _ = fs::remove_file(backup);
            }
            if let Err(restore_error) = restore_schematic.and(restore_metadata) {
                return Err(format!(
                    "Could not update schematic metadata: {error}. Rollback failed: {restore_error}"
                ));
            }
            return Err(error);
        }
        let _ = fs::remove_file(&schematic_backup);
        if let Some(backup) = metadata_backup {
            let _ = fs::remove_file(backup);
        }
        return Ok(existing);
    }
    let stem = safe_name_part(title, 48);
    let stem = if stem.is_empty() {
        format!("schematic_{}", safe_name_part(project_id, 16))
    } else {
        stem
    };
    let name = unique_name(&directory, &stem);
    let destination = schematic_path(&directory, &name);
    write_new_file(&destination, bytes)?;
    let metadata = SchematicMetadata {
        schema_version: 1,
        source: "shroudedit".into(),
        project_id: Some(project_id.to_owned()),
        version_id: Some(version_id.to_owned()),
        content_sha256: Some(incoming_hash),
        duplicate_of: None,
    };
    if let Err(error) = write_metadata(&directory, &name, &metadata) {
        let _ = fs::remove_file(destination);
        return Err(error);
    }
    Ok(name)
}

pub fn migrate_legacy_directory(
    root: &Path,
    check_cancelled: impl Fn() -> Result<(), String>,
) -> Result<usize, String> {
    let legacy = shroudforge_loader_config::paths::export_dir(root)
        .join("world-editor")
        .join("blueprints");
    let Some(_) = regular_directory_if_present(&legacy)? else {
        return Ok(0);
    };
    let destination = ensure_directory(root)?;
    let entries = fs::read_dir(&legacy)
        .map_err(|error| format!("Could not read the legacy blueprint folder: {error}"))?;
    let mut old_names = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Could not read a legacy blueprint entry: {error}"))?;
        let path = entry.path();
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("sfbp"))
        {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "Could not inspect legacy schematic {}: {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "Legacy schematic is not a regular file: {}",
                path.display()
            ));
        }
        let old_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or("Legacy schematic has an invalid name")?;
        old_names.push(old_name.to_owned());
    }
    old_names.sort_by_key(|value| value.to_lowercase());
    let mut moved_count = 0;
    for old_name in old_names {
        check_cancelled()?;
        let new_name = migrated_name(&legacy, &destination, &old_name)?;
        let mut files = Vec::new();
        for suffix in MIGRATION_SUFFIXES {
            let source = legacy.join(format!("{old_name}{suffix}"));
            if !path_exists(&source)? {
                continue;
            }
            let target = destination.join(format!("{new_name}{suffix}"));
            copy_or_verify_file(&source, &target, &check_cancelled)?;
            files.push((source, target, *suffix));
        }
        for (source, target, _) in &files {
            verify_identical_files(source, target)?;
        }
        for (source, target, _) in files.iter().filter(|(_, _, suffix)| *suffix != ".sfbp") {
            check_cancelled()?;
            verify_identical_files(source, target)?;
            fs::remove_file(source)
                .map_err(|error| format!("Could not remove verified legacy file: {error}"))?;
        }
        for (source, target, _) in files.iter().filter(|(_, _, suffix)| *suffix == ".sfbp") {
            check_cancelled()?;
            verify_identical_files(source, target)?;
            fs::remove_file(source)
                .map_err(|error| format!("Could not remove verified legacy schematic: {error}"))?;
        }
        moved_count += 1;
    }
    check_cancelled()?;
    if fs::read_dir(&legacy)
        .map_err(|error| format!("Could not verify the legacy blueprint folder: {error}"))?
        .next()
        .is_none()
    {
        fs::remove_dir(&legacy).map_err(|error| {
            format!("Could not remove the empty legacy blueprint folder: {error}")
        })?;
        if let Some(parent) = legacy.parent()
            && fs::read_dir(parent)
                .map_err(|error| {
                    format!("Could not inspect the legacy World Editor folder: {error}")
                })?
                .next()
                .is_none()
        {
            let _ = fs::remove_dir(parent);
        }
    }
    Ok(moved_count)
}

pub fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn validate_name(name: &str) -> Result<(), String> {
    if valid_name(name) {
        Ok(())
    } else {
        Err("Schematic names may contain only letters, numbers, underscores, and hyphens".into())
    }
}

fn validate_catalog_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        Err("Catalog schematic ID is invalid".into())
    } else {
        Ok(())
    }
}

fn validate_schematic_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 > MAX_SCHEMATIC_BYTES {
        return Err("Schematic exceeds the 256 MiB size limit".into());
    }
    crate::validate_schematic_v8::validate(bytes).map(|_| ())
}

fn cached_inspection(
    path: &Path,
    metadata: &fs::Metadata,
) -> crate::convert_schematic_v7::Inspection {
    let modified = metadata.modified().ok();
    let length = metadata.len();
    if let Ok(cache) = VALIDATION_CACHE.lock()
        && let Some((cached_length, cached_modified, inspection)) = cache.get(path)
        && *cached_length == length
        && *cached_modified == modified
    {
        return inspection.clone();
    }
    let inspection = if length > MAX_SCHEMATIC_BYTES {
        crate::convert_schematic_v7::Inspection {
            format_version: None,
            valid: false,
            convertible: false,
            validation_error: Some("Schematic exceeds the 256 MiB size limit".into()),
        }
    } else {
        match fs::read(path) {
            Ok(bytes) => crate::convert_schematic_v7::inspect(&bytes),
            Err(error) => crate::convert_schematic_v7::Inspection {
                format_version: None,
                valid: false,
                convertible: false,
                validation_error: Some(format!("Could not read schematic: {error}")),
            },
        }
    };
    if let Ok(mut cache) = VALIDATION_CACHE.lock() {
        cache.insert(path.to_path_buf(), (length, modified, inspection.clone()));
        if cache.len() > 2048
            && let Some(oldest) = cache
                .keys()
                .find(|candidate| candidate.as_path() != path)
                .cloned()
        {
            cache.remove(&oldest);
        }
    }
    inspection
}

fn clear_validation_cache(path: &Path) {
    if let Ok(mut cache) = VALIDATION_CACHE.lock() {
        cache.remove(path);
    }
}

fn ensure_directory(root: &Path) -> Result<PathBuf, String> {
    let directory = directory(root);
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("Schematics folder must be a regular directory".into());
        }
        Ok(_) => return Ok(directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("Could not inspect Schematics folder: {error}"));
        }
    }
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create Schematics folder: {error}"))?;
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|error| format!("Could not inspect Schematics folder: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Schematics folder must be a regular directory".into());
    }
    Ok(directory)
}

fn regular_directory_if_present(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "Storage folder is not a regular directory: {}",
            path.display()
        )),
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Could not inspect storage folder {}: {error}",
            path.display()
        )),
    }
}

fn regular_file_metadata(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "Could not inspect schematic file {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Schematic path must name a regular file".into());
    }
    Ok(metadata)
}

fn schematic_path(directory: &Path, name: &str) -> PathBuf {
    directory.join(format!("{name}.sfbp"))
}

fn associated_paths(directory: &Path, name: &str) -> Vec<PathBuf> {
    ASSOCIATED_SUFFIXES
        .iter()
        .map(|suffix| directory.join(format!("{name}{suffix}")))
        .collect()
}

fn read_metadata(directory: &Path, name: &str) -> Option<SchematicMetadata> {
    let path = directory.join(format!("{name}.metadata.json"));
    let metadata = fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 16 * 1024 {
        return None;
    }
    let record: SchematicMetadata = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    (record.schema_version == 1).then_some(record)
}

fn write_metadata(
    directory: &Path,
    name: &str,
    metadata: &SchematicMetadata,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(metadata).map_err(|error| error.to_string())?;
    write_replace_file(&directory.join(format!("{name}.metadata.json")), &bytes)
}

fn read_preview(directory: &Path, name: &str) -> Option<String> {
    for suffix in [".thumb.png", ".png"] {
        let path = directory.join(format!("{name}{suffix}"));
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > 2 * 1024 * 1024
        {
            continue;
        }
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        return Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ));
    }
    None
}

fn normalize_cover(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("Could not read cover image: {error}"))?;
    let format = reader
        .format()
        .ok_or("Cover image format could not be identified")?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err("Cover images must be PNG or JPEG files".into());
    }
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("Could not inspect cover image: {error}"))?;
    if width == 0
        || height == 0
        || width > MAX_COVER_DIMENSION
        || height > MAX_COVER_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_COVER_PIXELS
    {
        return Err("Cover image dimensions exceed the 8192 by 8192 limit".into());
    }
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|error| format!("Could not decode cover image: {error}"))?;
    let full = encode_png(&image)?;
    let thumbnail = encode_png(&image.thumbnail(960, 540))?;
    Ok((full, thumbnail))
}

fn encode_png(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| format!("Could not encode cover image: {error}"))?;
    Ok(output.into_inner())
}

fn backup_cover(directory: &Path, name: &str) -> Result<(), String> {
    let cover = directory.join(format!("{name}.png"));
    let thumbnail = directory.join(format!("{name}.thumb.png"));
    let cover_backup = directory.join(format!("{name}.cover-undo.png"));
    let thumbnail_backup = directory.join(format!("{name}.cover-undo.thumb.png"));
    let had_cover = cover.is_file();
    let had_thumbnail = thumbnail.is_file();
    if had_cover {
        copy_replace_file(&cover, &cover_backup)?;
    } else {
        let _ = fs::remove_file(&cover_backup);
    }
    if had_thumbnail {
        copy_replace_file(&thumbnail, &thumbnail_backup)?;
    } else {
        let _ = fs::remove_file(&thumbnail_backup);
    }
    let undo = serde_json::to_vec(&serde_json::json!({
        "hadCover": had_cover,
        "hadThumbnail": had_thumbnail,
    }))
    .map_err(|error| error.to_string())?;
    write_replace_file(&directory.join(format!("{name}.cover-undo.json")), &undo)
}

fn restore_cover_file(
    directory: &Path,
    current_name: &str,
    backup_name: &str,
    had_previous: bool,
) -> Result<(), String> {
    let current = directory.join(current_name);
    let backup = directory.join(backup_name);
    if had_previous {
        copy_replace_file(&backup, &current)
    } else {
        match fs::remove_file(current) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Could not remove the current cover: {error}")),
        }
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic(path, bytes, false)
}

fn write_replace_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic(path, bytes, true)
}

fn write_atomic(path: &Path, bytes: &[u8], replace: bool) -> Result<(), String> {
    let parent = path.parent().ok_or("Schematic file has no parent folder")?;
    let temp = parent.join(format!(
        ".schematic-{}-{}.tmp",
        std::process::id(),
        unique_stamp()
    ));
    let backup = parent.join(format!(
        ".schematic-{}-{}.bak",
        std::process::id(),
        unique_stamp()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| format!("Could not stage schematic file: {error}"))?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|error| format!("Could not write schematic file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Could not flush schematic file: {error}"))?;
        drop(file);
        if path.exists() {
            if !replace {
                return Err("Destination schematic file already exists".into());
            }
            regular_file_metadata(path)?;
            fs::rename(path, &backup)
                .map_err(|error| format!("Could not preserve existing schematic file: {error}"))?;
        }
        if let Err(error) = fs::rename(&temp, path) {
            if backup.exists() {
                let _ = fs::rename(&backup, path);
            }
            return Err(format!("Could not publish schematic file: {error}"));
        }
        if backup.exists() {
            fs::remove_file(&backup)
                .map_err(|error| format!("Could not remove preserved schematic file: {error}"))?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
        if backup.exists() && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
    }
    result
}

fn copy_new_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err("Destination schematic file already exists".into());
    }
    let source_metadata = regular_file_metadata(source)?;
    let mut input =
        File::open(source).map_err(|error| format!("Could not open source file: {error}"))?;
    let parent = destination
        .parent()
        .ok_or("Schematic file has no parent folder")?;
    let temp = parent.join(format!(
        ".schematic-{}-{}.tmp",
        std::process::id(),
        unique_stamp()
    ));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| format!("Could not stage schematic file: {error}"))?;
    let result = (|| {
        let copied = std::io::copy(&mut input, &mut output)
            .map_err(|error| format!("Could not copy schematic file: {error}"))?;
        if copied != source_metadata.len() {
            return Err("Schematic file changed during copying".into());
        }
        output
            .sync_all()
            .map_err(|error| format!("Could not flush schematic file: {error}"))?;
        drop(output);
        fs::rename(&temp, destination)
            .map_err(|error| format!("Could not publish schematic file: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn copy_replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    let source_metadata = regular_file_metadata(source)?;
    if source_metadata.len() > MAX_SCHEMATIC_BYTES {
        return Err("Source file exceeds the supported size".into());
    }
    let temp = destination.with_file_name(format!(
        ".schematic-{}-{}.tmp",
        std::process::id(),
        unique_stamp()
    ));
    fs::copy(source, &temp).map_err(|error| format!("Could not stage file copy: {error}"))?;
    let copied = fs::metadata(&temp)
        .map_err(|error| error.to_string())?
        .len();
    if copied != source_metadata.len() {
        let _ = fs::remove_file(&temp);
        return Err("Source file changed during copying".into());
    }
    let backup = destination.with_file_name(format!(
        ".schematic-{}-{}.bak",
        std::process::id(),
        unique_stamp()
    ));
    if destination.exists() {
        regular_file_metadata(destination)?;
        if let Err(error) = fs::rename(destination, &backup) {
            let _ = fs::remove_file(&temp);
            return Err(format!(
                "Could not preserve existing schematic file: {error}"
            ));
        }
    }
    if let Err(error) = fs::rename(&temp, destination) {
        let _ = fs::remove_file(&temp);
        if backup.exists() {
            let _ = fs::rename(&backup, destination);
        }
        return Err(format!("Could not publish copied file: {error}"));
    }
    if backup.exists() {
        fs::remove_file(&backup)
            .map_err(|error| format!("Could not remove preserved schematic file: {error}"))?;
    }
    Ok(())
}

fn move_regular_file(source: &Path, destination: &Path) -> Result<(), String> {
    regular_file_metadata(source)?;
    fs::rename(source, destination).map_err(|error| {
        format!(
            "Could not move schematic file {}: {error}",
            source.display()
        )
    })
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("Could not inspect {}: {error}", path.display())),
    }
}

fn copy_or_verify_file(
    source: &Path,
    destination: &Path,
    check_cancelled: &impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    if path_exists(destination)? {
        verify_identical_files(source, destination).map_err(|_| {
            format!(
                "Migration target already contains different or unsafe data: {}",
                destination.display()
            )
        })?;
        return Ok(());
    }
    check_cancelled()?;
    copy_new_file(source, destination)?;
    if let Err(error) = verify_identical_files(source, destination) {
        let _ = fs::remove_file(destination);
        return Err(format!(
            "Copied legacy schematic did not match its source: {error}"
        ));
    }
    Ok(())
}

fn verify_identical_files(source: &Path, destination: &Path) -> Result<(), String> {
    let source_metadata = regular_file_metadata(source)?;
    let destination_metadata = regular_file_metadata(destination)?;
    if source_metadata.len() != destination_metadata.len()
        || file_hash(source)? != file_hash(destination)?
    {
        return Err(format!(
            "Migration files do not match: {} and {}",
            source.display(),
            destination.display()
        ));
    }
    Ok(())
}

fn file_hash(path: &Path) -> Result<[u8; 32], String> {
    let mut file = File::open(path).map_err(|error| format!("Could not hash file: {error}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read file: {error}"))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize().into())
}

fn hash_hex(hash: [u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn migrated_name(legacy: &Path, destination: &Path, old_name: &str) -> Result<String, String> {
    let source_primary = legacy.join(format!("{old_name}.sfbp"));
    let source_primary_metadata = regular_file_metadata(&source_primary)?;
    let base = if valid_name(old_name) {
        old_name.to_owned()
    } else {
        let safe = safe_name_part(old_name, 48);
        if safe.is_empty() {
            "schematic".into()
        } else {
            format!("legacy_{safe}")
        }
    };
    for suffix in 0u32.. {
        let candidate = if suffix == 0 {
            base.clone()
        } else {
            let tail = format!("_{suffix}");
            format!("{}{tail}", &base[..base.len().min(64 - tail.len())])
        };
        let target_primary = destination.join(format!("{candidate}.sfbp"));
        let same_primary = if path_exists(&target_primary)? {
            let target_metadata = regular_file_metadata(&target_primary)?;
            source_primary_metadata.len() == target_metadata.len()
                && file_hash(&source_primary)? == file_hash(&target_primary)?
        } else {
            false
        };
        let mut compatible = true;
        for extension in ASSOCIATED_SUFFIXES {
            let source = legacy.join(format!("{old_name}{extension}"));
            let target = destination.join(format!("{candidate}{extension}"));
            let source_exists = match fs::symlink_metadata(&source) {
                Ok(_) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => {
                    return Err(format!(
                        "Could not inspect legacy file {}: {error}",
                        source.display()
                    ));
                }
            };
            let target_exists = match fs::symlink_metadata(&target) {
                Ok(_) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => {
                    return Err(format!(
                        "Could not inspect Schematics file {}: {error}",
                        target.display()
                    ));
                }
            };
            match (source_exists, target_exists) {
                (false, false) | (true, false) => {}
                (true, true) => {
                    let source_meta = regular_file_metadata(&source)?;
                    let target_meta = regular_file_metadata(&target)?;
                    if source_meta.len() != target_meta.len()
                        || file_hash(&source)? != file_hash(&target)?
                    {
                        compatible = false;
                        break;
                    }
                }
                (false, true) if same_primary => {}
                (false, true) => {
                    compatible = false;
                    break;
                }
            }
        }
        if compatible {
            return Ok(candidate);
        }
    }
    Err("Could not choose a unique name for a legacy schematic".into())
}

fn find_catalog_schematic(directory: &Path, project_id: &str) -> Result<Option<String>, String> {
    let Some(_) = regular_directory_if_present(directory)? else {
        return Ok(None);
    };
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("sfbp"))
        {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if read_metadata(directory, name).is_some_and(|metadata| {
            metadata.source == "shroudedit" && metadata.project_id.as_deref() == Some(project_id)
        }) {
            return Ok(Some(name.to_owned()));
        }
    }
    Ok(None)
}

fn unique_name(directory: &Path, base: &str) -> String {
    for suffix in 0u32.. {
        let candidate = if suffix == 0 {
            base.to_owned()
        } else {
            let tail = format!("_{suffix}");
            format!("{}{tail}", &base[..base.len().min(64 - tail.len())])
        };
        if associated_paths(directory, &candidate)
            .iter()
            .all(|path| !path.exists())
        {
            return candidate;
        }
    }
    unreachable!("finite filesystem name space exhausted")
}

fn safe_name_part(value: &str, limit: usize) -> String {
    value
        .bytes()
        .filter(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        .take(limit)
        .map(char::from)
        .collect()
}

fn unique_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
