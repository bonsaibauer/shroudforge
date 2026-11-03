use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn read_package_file(package: &Path, relative: &str) -> Result<Vec<u8>, String> {
    if relative.is_empty()
        || relative.contains("..")
        || relative.contains('\\')
        || Path::new(relative).is_absolute()
    {
        return Err("invalid package path".into());
    }
    if package.is_dir() {
        return fs::read(package.join(relative)).map_err(|error| error.to_string());
    }
    if !package
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Err("unsupported mod package".into());
    }
    let file = File::open(package).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut entry = archive
        .by_name(relative)
        .map_err(|error| error.to_string())?;
    if entry.size() > 2 * 1024 * 1024 {
        return Err("package file is too large".into());
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

pub fn package_has_file(package: &Path, relative: &str) -> Result<bool, String> {
    if package.is_dir() {
        return Ok(package.join(relative).is_file());
    }
    let file = File::open(package).map_err(|error| error.to_string())?;
    let archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    for name in archive.file_names() {
        if name.map_err(|error| error.to_string())? == relative {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn write_package_json(
    package: &Path,
    relative: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    if package.is_dir() {
        let path = package.join(relative);
        let parsed = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        return crate::config::write_json(&path, &parsed);
    }
    write_zip_entry(package, relative, &bytes)
}

pub fn write_zip_entry(package: &Path, relative: &str, replacement: &[u8]) -> Result<(), String> {
    let source = File::open(package).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(source).map_err(|error| error.to_string())?;
    let temp = package.with_file_name(format!(
        ".shroudforge-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    let mut writer = zip::ZipWriter::new(target);
    let mut found = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry
            .name()
            .map_err(|error| error.to_string())?
            .into_owned();
        let is_dir = entry.is_dir();
        let mode = entry.unix_mode();
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        if name == relative {
            writer
                .start_file(name, options)
                .map_err(|error| error.to_string())?;
            writer
                .write_all(replacement)
                .map_err(|error| error.to_string())?;
            found = true;
        } else if is_dir {
            writer
                .add_directory(name, options)
                .map_err(|error| error.to_string())?;
        } else {
            writer
                .start_file(
                    name,
                    if let Some(mode) = mode {
                        options.unix_permissions(mode)
                    } else {
                        options
                    },
                )
                .map_err(|error| error.to_string())?;
            std::io::copy(&mut entry, &mut writer).map_err(|error| error.to_string())?;
        }
    }
    if !found {
        writer
            .start_file(
                relative,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .map_err(|error| error.to_string())?;
        writer
            .write_all(replacement)
            .map_err(|error| error.to_string())?;
    }
    let file = writer.finish().map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(archive);
    let backup = package.with_file_name(format!(
        ".shroudforge-{}-{}.bak",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::rename(package, &backup).map_err(|error| {
        let _ = fs::remove_file(&temp);
        error.to_string()
    })?;
    if let Err(error) = fs::rename(&temp, package) {
        let _ = fs::rename(&backup, package);
        let _ = fs::remove_file(&temp);
        return Err(error.to_string());
    }
    fs::remove_file(backup).map_err(|error| error.to_string())
}
