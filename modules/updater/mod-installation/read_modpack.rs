use shroudforge_package::modpack::{ModpackManifest, PackMod};
use std::{collections::HashSet, io::{Read, Seek, SeekFrom}, sync::LazyLock};

pub const MAX_ARCHIVE_BYTES: u64 = 95 * 1024 * 1024;
pub const MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 10_000;
pub const MANIFEST_NAME: &str = "modpack.json";

pub fn read_modpack<R: Read + Seek>(mut input: R) -> Result<ModpackManifest, String> {
    let size = input.seek(SeekFrom::End(0)).map_err(|error| error.to_string())?;
    if size > MAX_ARCHIVE_BYTES {
        return Err(".sfpack exceeds 95 MiB".into());
    }
    input.seek(SeekFrom::Start(0)).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(input).map_err(|error| error.to_string())?;
    if archive.len() > MAX_ENTRIES {
        return Err(".sfpack contains more than 10,000 entries".into());
    }
    let manifest_bytes = read_json_entry(&mut archive, MANIFEST_NAME)?;
    let pack = shroudforge_package::modpack::parse_manifest(&manifest_bytes)?;
    let bundled = pack.mods.iter().filter_map(|item| match item {
        PackMod::Bundled { path, .. } => Some(path.as_str()),
        _ => None,
    }).collect::<Vec<_>>();
    let mut names = HashSet::new();
    let mut payloads = HashSet::new();
    let mut expanded = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().map_err(|error| error.to_string())?.into_owned();
        let path = name.trim_end_matches('/');
        if !portable_archive_path(path)
            || !names.insert(path.to_ascii_lowercase())
            || entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000)
            || !matches!(entry.compression(), zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated)
        {
            return Err(format!("invalid .sfpack entry {name}"));
        }
        let owner = bundled.iter().find(|root| path == **root || path.strip_prefix(**root).is_some_and(|rest| rest.starts_with('/')));
        if path != MANIFEST_NAME && owner.is_none() && !(entry.is_dir() && path == "mods") {
            return Err(format!("undeclared .sfpack entry {name}"));
        }
        if owner.is_some_and(|root| path == *root) && !entry.is_dir() {
            return Err(format!("bundled mod root {name} must be a directory"));
        }
        if path == MANIFEST_NAME && entry.is_dir() {
            return Err("modpack.json must be a file".into());
        }
        if entry.is_dir() { continue }
        if let Some(root) = owner { payloads.insert(*root); }
        expanded = expanded.checked_add(entry.size()).ok_or("expanded .sfpack size overflow")?;
        if expanded > MAX_EXPANDED_BYTES { return Err("expanded .sfpack exceeds 512 MiB".into()) }
        let copied = std::io::copy(&mut entry.take(MAX_EXPANDED_BYTES + 1), &mut std::io::sink())
            .map_err(|error| error.to_string())?;
        if copied > MAX_EXPANDED_BYTES { return Err(format!(".sfpack entry {name} exceeds 512 MiB")) }
    }
    static MOD_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(
            include_str!("../../modloader/manifest/manifest.schema.json"))
            .expect("bundled mod schema is JSON")).expect("bundled mod schema is valid")
    });
    static EXTENSION_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(
            include_str!("../../modloader/manifest/extended.mod.schema.json"))
            .expect("bundled extension schema is JSON")).expect("bundled extension schema is valid")
    });
    for root in bundled {
        if !payloads.contains(root) { return Err(format!("bundled mod folder {root} is empty or missing")) }
        let mod_path = format!("{root}/mod.json");
        if names.contains(&mod_path.to_ascii_lowercase()) {
            let value: serde_json::Value = serde_json::from_slice(&read_json_entry(&mut archive, &mod_path)?)
                .map_err(|error| format!("{mod_path}: {error}"))?;
            MOD_VALIDATOR.validate(&value).map_err(|error| format!("{mod_path}: {error}"))?;
            if value["id"].as_str() != root.rsplit('/').next() {
                return Err(format!("{mod_path} id differs from its folder"));
            }
        }
        let extension_path = format!("{root}/extended.mod.json");
        if names.contains(&extension_path.to_ascii_lowercase()) {
            let value: serde_json::Value = serde_json::from_slice(&read_json_entry(&mut archive, &extension_path)?)
                .map_err(|error| format!("{extension_path}: {error}"))?;
            EXTENSION_VALIDATOR.validate(&value).map_err(|error| format!("{extension_path}: {error}"))?;
        }
    }
    Ok(pack)
}

fn read_json_entry<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Result<Vec<u8>, String> {
    let mut entry = archive.by_name(name).map_err(|_| format!("missing {name}"))?;
    if entry.is_dir() || entry.size() > 1024 * 1024 { return Err(format!("invalid {name}")) }
    let mut bytes = Vec::new();
    entry.by_ref().take(1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() > 1024 * 1024 { return Err(format!("{name} exceeds 1 MiB")) }
    Ok(bytes)
}

fn portable_archive_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 4096 || path.starts_with('/') || path.contains('\\')
        || path.chars().any(|character| character.is_control() || matches!(character, ':' | '<' | '>' | '"' | '?' | '*' | '|')) {
        return false;
    }
    path.split('/').all(|part| {
        if part.is_empty() || matches!(part, "." | "..") || part.ends_with(' ') || part.ends_with('.') { return false }
        let base = part.split('.').next().unwrap_or_default().to_ascii_uppercase();
        !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            && !(base.len() == 4 && (base.starts_with("COM") || base.starts_with("LPT"))
                && base.as_bytes()[3].is_ascii_digit() && base.as_bytes()[3] != b'0')
    })
}
