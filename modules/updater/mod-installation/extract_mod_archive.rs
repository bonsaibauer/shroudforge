use crate::mod_package::{MAX_ARCHIVE_ENTRIES, MAX_EXTRACTED_BYTES};
use std::{
    fs::{self, File},
    path::{Component, Path},
};

pub fn extract_zip(root: &Path, archive_path: &Path, target: &Path) -> Result<(), String> {
    crate::check_update_cancelled::check_update_cancelled(root)?;
    let file = File::open(archive_path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Update archive contains too many entries".into());
    }
    let mut expanded = 0_u64;
    for index in 0..archive.len() {
        crate::check_update_cancelled::check_update_cancelled(root)?;
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let entry_name = entry
            .name()
            .map_err(|error| error.to_string())?
            .into_owned();
        let Some(relative) = entry.enclosed_name() else {
            return Err(format!("Unsicherer Archivpfad: {entry_name}"));
        };
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(format!("Unsicherer Archivpfad: {entry_name}"));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!(
                "Update archive contains a disallowed symbolic link: {}",
                entry_name
            ));
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > MAX_EXTRACTED_BYTES {
            return Err("Extracted update exceeds 2 GiB".into());
        }
        let output = target.join(relative);
        ensure_child(target, &output)?;
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut destination = File::create(output).map_err(|error| error.to_string())?;
        crate::apply_release_files::copy_reader_with_cancellation(
            &mut entry,
            &mut destination,
            root,
            "mod archive extraction",
        )?;
    }
    crate::check_update_cancelled::check_update_cancelled(root)?;
    Ok(())
}

pub fn ensure_child(root: &Path, child: &Path) -> Result<(), String> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let candidate = child.canonicalize().unwrap_or_else(|_| child.to_path_buf());
    if !candidate.starts_with(&root) {
        return Err(format!("Path resolves outside {}", root.display()));
    }
    Ok(())
}
