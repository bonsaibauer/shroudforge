use crate::config::revision;
use std::{io::Read, path::Path};
pub fn manifest_revision(package: &Path) -> Result<String, String> {
    let path = camino::Utf8Path::from_path(package).ok_or("package path is not UTF-8")?;
    let mut package_fs = if package.is_dir() {
        crate::FileSystem::new_disk(path)
    } else {
        crate::FileSystem::new_zip(path)
    }
    .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    for name in ["mod.json", "extended.mod.json"] {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        match package_fs.open_package_file(name) {
            Ok(mut file) => {
                file.read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => bytes.push(0xff),
            Err(error) => return Err(format!("{}: {error}", package.display())),
        }
        bytes.push(0);
    }
    if package_fs.exists("locales") {
        if !package_fs.is_directory("locales") {
            return Err(format!(
                "{}: locales must be a directory",
                package.display()
            ));
        }
        let mut locale_files = package_fs
            .read_directory("locales")
            .map_err(|error| format!("{}: locales: {error}", package.display()))?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("{}: locales: {error}", package.display()))?;
        locale_files.retain(|path| path.extension() == Some("json"));
        locale_files.sort();
        for path in locale_files {
            if package_fs.is_directory(&path) {
                continue;
            }
            bytes.extend_from_slice(path.as_str().as_bytes());
            bytes.push(0);
            match package_fs.open_package_file(&path) {
                Ok(mut file) => {
                    file.read_to_end(&mut bytes)
                        .map_err(|error| format!("{}: {path}: {error}", package.display()))?;
                }
                Err(error) => return Err(format!("{}: {path}: {error}", package.display())),
            }
            bytes.push(0);
        }
    }
    Ok(revision(&bytes))
}
