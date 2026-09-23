use crate::ModEnvironment;
use serde_json::Value;
use std::collections::HashMap;

pub fn conflicts(env: &ModEnvironment) -> Result<HashMap<String,String>,String> {
    let root = env.game_dir().as_std_path();
    let path = crate::config::document_path(root,"rules");
    let value: Value = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::from_str(include_str!("../../../config/compatibility/rules.json")).map_err(|e| e.to_string())?,
        Err(error) => return Err(error.to_string()),
    };
    crate::config::validate_document(root,"rules",&value)?;
    let mut blocked = HashMap::new();
    for rule in value["conflicts"].as_array().ok_or("conflicts must be an array")? {
        let first = &rule["first"];
        let second = &rule["second"];
        let matches = |constraint: &Value| -> Result<bool,String> {
            let id = constraint["id"].as_str().ok_or("missing conflict id")?;
            let version = semver::VersionReq::parse(constraint["version"].as_str().ok_or("missing version range")?).map_err(|e| e.to_string())?;
            Ok(env.is_mod_enabled(id) && env.mod_registry().get(id).is_some_and(|item| version.matches(&item.info().version)))
        };
        let a = matches(first)?;
        let b = matches(second)?;
        if a && b {
            let reason = rule["reason"].as_str().ok_or("missing conflict reason")?;
            blocked.insert(first["id"].as_str().unwrap().into(), reason.into());
            blocked.insert(second["id"].as_str().unwrap().into(), reason.into());
        }
    }
    Ok(blocked)
}
