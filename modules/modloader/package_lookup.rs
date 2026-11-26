use crate::package_files::{package_has_file, read_package_file};
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

pub fn mod_ecosystem(root: &Path, id: &str) -> Option<&'static str> {
    let package = find_mod_package(root, id)?;
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
