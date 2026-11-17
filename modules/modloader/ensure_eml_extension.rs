use crate::package_files::{package_has_file, read_package_file};
use std::path::Path;

pub fn ensure_eml_extension(
    package: &Path,
    manifest: &crate::ModManifest,
) -> Result<serde_json::Value, String> {
    let mut extension = match read_package_file(package, "extended.mod.json") {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|error| error.to_string())?,
        Err(_error) if !package_has_file(package, "extended.mod.json")? => {
            serde_json::json!({"$schema":"https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json","schemaVersion":1,"enabled":manifest.enabled,"targets":["client","server"],"launcher":"EML","settings":{}})
        }
        Err(error) => return Err(error),
    };
    extension
        .as_object_mut()
        .ok_or("extended.mod.json must be an object")?
        .entry("settings")
        .or_insert_with(|| serde_json::json!({}));
    let targets = if extension.get("launcher").and_then(|value| value.as_str()) == Some("EML") {
        serde_json::json!(["client", "server"])
    } else {
        serde_json::json!(["client"])
    };
    let object = extension
        .as_object_mut()
        .ok_or("extended.mod.json must be an object")?;
    if !object.contains_key("targets") {
        let old = std::mem::take(object);
        let mut migrated = serde_json::Map::new();
        for (key, value) in old {
            let insert_target = key == "enabled";
            migrated.insert(key, value);
            if insert_target {
                migrated.insert("targets".into(), targets.clone());
            }
        }
        *object = migrated;
    }
    Ok(extension)
}
