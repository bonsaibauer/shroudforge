use crate::config::update_loader;
use serde_json::Value;
use std::path::Path;

pub fn initialize_loader_config(root: &Path) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("could not resolve game installation root: {error}"))?;
    update_loader(&root, |config| {
        for key in [
            "mods", "state", "logs", "cache", "exports", "updates", "ui", "runtime",
        ] {
            let path = config
                .pointer(&format!("/paths/{key}"))
                .cloned()
                .ok_or_else(|| format!("missing storage path setting: {key}"))?;
            let resolved = match path {
                Value::Null => crate::paths::default_directory(&root, key)
                    .ok_or_else(|| format!("unknown storage path: {key}"))?,
                Value::String(value) => {
                    let path = Path::new(&value);
                    if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        let joined = root.join(path);
                        joined.canonicalize().unwrap_or(joined)
                    }
                }
                _ => return Err(format!("invalid storage path setting: {key}")),
            };
            config["paths"][key] = Value::String(resolved.to_string_lossy().into_owned());
        }
        Ok(())
    })
}
