use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::{Read, Seek}, sync::LazyLock};

pub const MAX_PACK_BYTES: u64 = 1024 * 1024;
pub const MANIFEST_NAME: &str = "shroudforge.index.json";
pub const SCHEMA: &str = include_str!("schemas/sfpack/1/manifest.schema.json");

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Modpack {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub format_version: u32,
    pub game: String,
    pub name: String,
    pub version_id: String,
    pub summary: Option<String>,
    pub game_versions: Vec<String>,
    pub mods: Vec<PackMod>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackMod {
    pub project_id: String,
    pub version_id: String,
    pub sha512: String,
}

pub fn parse_manifest(bytes: &[u8]) -> Result<Modpack, String> {
    if bytes.len() as u64 > MAX_PACK_BYTES {
        return Err("modpack manifest exceeds 1 MiB".into());
    }
    static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(SCHEMA)
            .expect("bundled sfpack schema is JSON"))
            .expect("bundled sfpack schema is valid")
    });
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if let Err(error) = VALIDATOR.validate(&value) {
        return Err(format!("invalid sfpack manifest at {}: {error}", error.instance_path));
    }
    let pack: Modpack = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut projects = HashSet::new();
    for item in &pack.mods {
        if !projects.insert(&item.project_id) {
            return Err(format!("duplicate modpack project `{}`", item.project_id));
        }
    }
    Ok(pack)
}

pub fn read_modpack<R: Read + Seek>(input: R) -> Result<Modpack, String> {
    let mut zip = zip::ZipArchive::new(input).map_err(|e| e.to_string())?;
    if zip.len() != 1 {
        return Err("sfpack must contain only shroudforge.index.json".into());
    }
    let mut entry = zip.by_name(MANIFEST_NAME).map_err(|_| "missing shroudforge.index.json")?;
    if entry.is_dir() || entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000)
        || !matches!(entry.compression(), zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated)
        || entry.size() > MAX_PACK_BYTES
    {
        return Err("invalid sfpack manifest entry".into());
    }
    let mut bytes = Vec::new();
    (&mut entry).take(MAX_PACK_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    parse_manifest(&bytes)
}

/// Validate a complete pinned set against v3 version metadata. Catalog readers
/// and the installer both enforce this contract before writing any package.
pub fn validate_versions(pack: &Modpack, versions: &[serde_json::Value]) -> Result<(), String> {
    for item in &pack.mods {
        let version = versions.iter().find(|version| version["id"].as_str() == Some(&item.version_id))
            .ok_or_else(|| format!("modpack version `{}` is unavailable", item.version_id))?;
        if version["project_id"].as_str() != Some(&item.project_id)
            || !matches!(version["status"].as_str(), Some("listed" | "archived" | "unlisted"))
            || !contains(&version["project_types"], "mod")
            || !(contains(&version["loaders"], "shroudforge") || contains(&version["loaders"], "eml"))
        {
            return Err(format!("version `{}` is not a public ShroudForge/EML mod of project `{}`", item.version_id, item.project_id));
        }
        if !pack.game_versions.iter().all(|game| contains(&version["game_versions"], game)) {
            return Err(format!("mod `{}` does not support every game version declared by the pack", item.project_id));
        }
        let file = version["files"].as_array().and_then(|files| {
            files.iter().find(|file| file["primary"].as_bool() == Some(true))
                .or_else(|| files.first())
        }).ok_or_else(|| format!("version `{}` has no primary package", item.version_id))?;
        if !file["hashes"]["sha512"].as_str().is_some_and(|hash| hash.eq_ignore_ascii_case(&item.sha512)) {
            return Err(format!("pinned ZIP checksum does not match the selected package of version `{}`", item.version_id));
        }
        if !file["filename"].as_str().is_some_and(|name| name.to_ascii_lowercase().ends_with(".zip"))
            || !file["url"].as_str().is_some_and(|url| url.starts_with("https://"))
            || !file["size"].as_u64().is_some_and(|size| size > 0 && size <= 512 * 1024 * 1024)
        {
            return Err(format!("version `{}` does not contain a supported mod ZIP", item.version_id));
        }
        for dependency in version["dependencies"].as_array().into_iter().flatten() {
            let project_id = dependency["project_id"].as_str();
            let version_id = dependency["version_id"].as_str();
            let matches = pack.mods.iter().any(|candidate| {
                (project_id.is_some() || version_id.is_some())
                    && project_id.is_none_or(|id| candidate.project_id == id)
                    && version_id.is_none_or(|id| candidate.version_id == id)
            });
            match dependency["dependency_type"].as_str() {
                Some("required") if !matches => return Err(format!(
                    "mod `{}` requires an unlisted or conflicting dependency `{}`", item.project_id,
                    version_id.or(project_id).unwrap_or("unknown")
                )),
                Some("incompatible") if matches => return Err(format!(
                    "mod `{}` conflicts with another pinned pack mod", item.project_id
                )),
                _ => {},
            }
        }
    }
    Ok(())
}

fn contains(value: &serde_json::Value, expected: &str) -> bool {
    value.as_array().is_some_and(|values| values.iter().any(|value| value.as_str() == Some(expected)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn example() -> serde_json::Value {
        serde_json::from_str(include_str!("schemas/sfpack/1/manifest.example.json")).unwrap()
    }

    #[test]
    fn schema_rejects_foreign_paths_formats_and_unpinned_mods() {
        assert!(parse_manifest(&serde_json::to_vec(&example()).unwrap()).is_ok());
        for (field, value) in [("game", json!("minecraft")), ("formatVersion", json!(2)), ("overrides", json!([])), ("mods", json!([]))] {
            let mut data = example();
            data[field] = value;
            assert!(parse_manifest(&serde_json::to_vec(&data).unwrap()).is_err(), "{field}");
        }
        let mut data = example();
        data["mods"][0]["projectId"] = json!("../escape");
        assert!(parse_manifest(&serde_json::to_vec(&data).unwrap()).is_err());
        let mut data = example();
        data["mods"][0].as_object_mut().unwrap().remove("versionId");
        assert!(parse_manifest(&serde_json::to_vec(&data).unwrap()).is_err());
        let mut data = example();
        let mut duplicate = data["mods"][0].clone();
        duplicate["versionId"] = json!("OTHER");
        data["mods"].as_array_mut().unwrap().push(duplicate);
        assert!(parse_manifest(&serde_json::to_vec(&data).unwrap()).is_err());
    }

    #[test]
    fn catalog_validation_requires_exact_file_and_complete_dependencies() {
        let pack = parse_manifest(&serde_json::to_vec(&example()).unwrap()).unwrap();
        let version = json!({"id":"VERSIONID","project_id":"PROJECTID","status":"listed",
            "project_types":["mod"],"loaders":["eml"],"game_versions":["REPLACE"],
            "files":[{"filename":"mod.zip","url":"https://cdn.shroudedit.com/mod.zip","size":100,"hashes":{"sha512": "0".repeat(128)}}], "dependencies":[]});
        assert!(validate_versions(&pack, &[version.clone()]).is_ok());
        let mut missing = version.clone();
        missing["dependencies"] = json!([{"project_id":"MISSING", "dependency_type":"required"}]);
        assert!(validate_versions(&pack, &[missing]).is_err());
        let mut wrong_hash = version.clone();
        wrong_hash["files"][0]["hashes"]["sha512"] = json!("a".repeat(128));
        assert!(validate_versions(&pack, &[wrong_hash]).is_err());
        let mut wrong_owner = version.clone();
        wrong_owner["project_id"] = json!("OTHER");
        assert!(validate_versions(&pack, &[wrong_owner]).is_err());
        let mut draft = version;
        draft["status"] = json!("draft");
        assert!(validate_versions(&pack, &[draft]).is_err());
    }
}
