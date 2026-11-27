use crate::package_files::{package_has_file, read_package_file};
use crate::modpack::{package_by_key, package_locations};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn find_mod_package(root: &Path, id: &str) -> Option<PathBuf> {
    fs::read_dir(shroudforge_loader_config::paths::mods_dir(root))
        .ok()?
        .flatten()
        .find_map(|entry| {
            let package = entry.path();
            let manifest = read_package_file(&package, "mod.json").ok()?;
            let value = serde_json::from_slice::<serde_json::Value>(&manifest).ok()?;
            (value.get("id").and_then(|value| value.as_str()) == Some(id)).then_some(package)
        })
}

pub fn find_mod_package_by_reference(root: &Path, reference: &str) -> Option<PathBuf> {
    if reference.starts_with("pack:") || reference.starts_with("direct:") {
        return package_by_key(root, reference).map(|location| location.path);
    }
    find_mod_package(root, reference)
}

pub fn mod_packages_with_id(root: &Path, id: &str) -> Vec<PathBuf> {
    package_locations(&shroudforge_loader_config::paths::mods_dir(root))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|location| {
            let manifest = read_package_file(&location.path, "mod.json").ok()?;
            let value: serde_json::Value = serde_json::from_slice(&manifest).ok()?;
            (value.get("id").and_then(|value| value.as_str()) == Some(id)).then_some(location.path)
        })
        .collect()
}

pub fn mod_ecosystem(root: &Path, id: &str) -> Option<&'static str> {
    let package = find_mod_package_by_reference(root, id)?;
    Some(mod_ecosystem_for_package(&package))
}

pub fn mod_ecosystem_for_package(package: &Path) -> &'static str {
    // ShroudForge mods always ship an extension. A package that only has
    // mod.json is an EML mod until ShroudForge creates the extension when
    // the user first changes its state.
    if !package_has_file(package, "extended.mod.json").unwrap_or(true) {
        return "EML";
    }
    let launcher = read_package_file(package, "extended.mod.json")
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| {
            value
                .get("launcher")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    match launcher.as_deref() {
        Some("EML") => "EML",
        // Existing extensions without EML provenance belong to SF.
        _ => "SF",
    }
}

pub fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

pub fn valid_mod_identifier(value: &str) -> bool {
    value.len() <= 64 && valid_identifier(value)
}

pub fn valid_package_reference(value: &str) -> bool {
    if valid_identifier(value) { return true }
    if let Some(id) = value.strip_prefix("direct:") { return valid_identifier(id) }
    value.strip_prefix("pack:").and_then(|rest| rest.split_once(':'))
        .is_some_and(|(pack, id)| valid_mod_identifier(pack) && valid_mod_identifier(id))
}
