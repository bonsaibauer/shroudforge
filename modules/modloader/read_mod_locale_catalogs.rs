use crate::{FileSystem, ModLocaleCatalog};
use camino::Utf8PathBuf;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub(crate) fn locale_catalog_files(fs: &mut FileSystem) -> Result<Vec<Utf8PathBuf>, String> {
    if !fs.exists("locales") {
        return Ok(Vec::new());
    }
    if !fs.is_directory("locales") {
        return Err("locales must be a directory".into());
    }
    let files = fs
        .read_directory("locales")
        .map_err(|error| format!("locales: {error}"))?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("locales: {error}"))?;
    let mut catalogs = Vec::new();
    for path in files {
        if path.extension() == Some("json") && !fs.is_directory(&path) {
            catalogs.push(path);
        }
    }
    catalogs.sort();
    Ok(catalogs)
}
pub(crate) fn read_mod_locale_catalogs(
    root: &Path,
    fs: &mut FileSystem,
) -> Result<BTreeMap<String, ModLocaleCatalog>, String> {
    let files = locale_catalog_files(fs)?;
    let mut catalogs = BTreeMap::new();
    let mut normalized_codes = HashSet::new();
    for path in files {
        let locale = path
            .file_stem()
            .filter(|locale| valid_locale_code(locale))
            .ok_or_else(|| format!("{path}: invalid locale filename"))?;
        if !normalized_codes.insert(locale.to_ascii_lowercase()) {
            return Err(format!("locales: duplicate locale code '{locale}'"));
        }
        // Buffered: serde_json reads a bare file one byte per system call.
        let mut value: Value = serde_json::from_reader(std::io::BufReader::new(
            fs.read_file(&path)
                .map_err(|error| format!("{path}: {error}"))?,
        ))
        .map_err(|error| format!("{path}: {error}"))?;
        shroudforge_loader_config::config::validate_document(
            root,
            crate::documents::MOD_LOCALE,
            &value,
        )
        .map_err(|error| format!("{path}: {error}"))?;
        if let Some(object) = value.as_object_mut() {
            object.remove("$schema");
        }
        let catalog = serde_json::from_value(value).map_err(|error| format!("{path}: {error}"))?;
        catalogs.insert(locale.to_owned(), catalog);
    }
    Ok(catalogs)
}

pub(crate) fn valid_locale_code(locale: &str) -> bool {
    (2..=35).contains(&locale.len())
        && !locale.starts_with('-')
        && !locale.ends_with('-')
        && !locale.contains("--")
        && locale
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && locale.split('-').all(|part| !part.is_empty())
}
