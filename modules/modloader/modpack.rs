use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::{Path, PathBuf}, sync::LazyLock};

pub const SCHEMA: &str = include_str!("manifest/modpack.schema.json");

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub game_versions: Vec<String>,
    pub mods: Vec<PackMod>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PackMod {
    Bundled { path: String, #[serde(default)] optional: bool },
    External { id: String, version: String, url: String, sha512: String, #[serde(default)] optional: bool },
    Catalog { id: String, version: String, #[serde(default)] optional: bool },
}

impl PackMod {
    pub fn id(&self) -> &str {
        match self {
            Self::Bundled { path, .. } => path.rsplit('/').next().unwrap_or(path),
            Self::External { id, .. } | Self::Catalog { id, .. } => id,
        }
    }

    pub fn optional(&self) -> bool {
        match self {
            Self::Bundled { optional, .. } | Self::External { optional, .. } | Self::Catalog { optional, .. } => *optional,
        }
    }

    pub fn source(&self) -> &'static str {
        match self {
            Self::Bundled { .. } => "bundled",
            Self::External { .. } => "external",
            Self::Catalog { .. } => "shroudedit",
        }
    }
}

pub fn parse_manifest(bytes: &[u8]) -> Result<ModpackManifest, String> {
    if bytes.len() > 1024 * 1024 {
        return Err("modpack.json exceeds 1 MiB".into());
    }
    static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(SCHEMA)
            .expect("bundled modpack schema is JSON"))
            .expect("bundled modpack schema is valid")
    });
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    VALIDATOR.validate(&value).map_err(|error| format!("invalid modpack.json at {}: {error}", error.instance_path))?;
    let manifest: ModpackManifest = serde_json::from_value(value).map_err(|error| error.to_string())?;
    let mut ids = HashSet::new();
    for item in &manifest.mods {
        if !ids.insert(item.id().to_ascii_lowercase()) {
            return Err(format!("duplicate mod '{}' in modpack.json", item.id()));
        }
    }
    Ok(manifest)
}

#[derive(Clone, Debug)]
pub struct InstalledPack {
    pub path: PathBuf,
    pub manifest: ModpackManifest,
}

pub fn installed_packs(mods_dir: &Path) -> Vec<InstalledPack> {
    let Ok(entries) = fs::read_dir(mods_dir) else { return Vec::new() };
    let mut packs = entries.flatten().filter_map(|entry| {
        if !entry.file_type().ok()?.is_dir() || entry.file_name().to_string_lossy().starts_with('.') { return None }
        let path = entry.path();
        let manifest = parse_manifest(&fs::read(path.join("modpack.json")).ok()?).ok()?;
        (path.file_name()?.to_str()? == manifest.id).then_some(InstalledPack { path, manifest })
    }).collect::<Vec<_>>();
    packs.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    packs
}

#[derive(Clone, Debug)]
pub struct PackageLocation {
    pub path: PathBuf,
    pub pack_id: Option<String>,
    pub id: String,
    pub pack_ready: bool,
}

impl PackageLocation {
    pub fn key(&self) -> String {
        match &self.pack_id {
            Some(pack) => format!("pack:{pack}:{}", self.id),
            None => format!("direct:{}", self.id),
        }
    }
}

pub fn package_locations(mods_dir: &Path) -> Result<Vec<PackageLocation>, String> {
    let mut packages = Vec::new();
    let entries = match fs::read_dir(mods_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(packages),
        Err(error) => return Err(error.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().map_err(|error| error.to_string())?.is_dir()
            || entry.file_name().to_string_lossy().starts_with('.') { continue }
        let path = entry.path();
        if path.join("modpack.json").exists() { continue }
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else { continue };
        packages.push(PackageLocation { path, pack_id: None, id, pack_ready: true });
    }
    for pack in installed_packs(mods_dir) {
        let ready = pack.manifest.mods.iter().filter(|item| !item.optional())
            .all(|item| pack.path.join("mods").join(item.id()).is_dir());
        for item in &pack.manifest.mods {
            let id = item.id();
            let path = pack.path.join("mods").join(id);
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink()) {
                packages.push(PackageLocation { path, pack_id: Some(pack.manifest.id.clone()), id: id.into(), pack_ready: ready });
            }
        }
    }
    packages.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(packages)
}

pub fn package_by_key(root: &Path, key: &str) -> Option<PackageLocation> {
    package_locations(&shroudforge_loader_config::paths::mods_dir(root)).ok()?.into_iter()
        .find(|location| location.key() == key)
}
