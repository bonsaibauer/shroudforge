use serde_json::Value;
use std::path::Path;

pub fn migrate_legacy_toggle_key_settings(root: &Path) -> Result<bool, String> {
    let _lock = crate::config::installation_lock(root)?;
    let path = crate::config::document_path(root, crate::documents::LOADER);
    let mut settings = crate::config::read_loader_unvalidated(root)?;
    let Some(modules) = settings.get_mut("modules").and_then(Value::as_object_mut) else {
        return Ok(false);
    };

    let mut changed = false;
    for (module, settings) in modules {
        let Some(binding) = settings.get_mut("toggleKey") else {
            continue;
        };
        if !binding.is_number() {
            continue;
        }
        let virtual_key = binding.as_u64().ok_or_else(|| {
            format!("{module}.toggleKey: legacy Windows key code must be a positive integer")
        })?;
        let virtual_key = u16::try_from(virtual_key)
            .map_err(|_| format!("{module}.toggleKey: legacy Windows key code is out of range"))?;
        let replacement = shroudforge_hotkeys::legacy_virtual_key_binding(virtual_key)
            .ok_or_else(|| {
                format!(
                    "{module}.toggleKey: legacy Windows key code {virtual_key} cannot be mapped to an SDL binding"
                )
            })?;
        *binding = Value::String(replacement);
        changed = true;
    }

    if !changed {
        return Ok(false);
    }
    crate::config::validate_document(root, crate::documents::LOADER, &settings)?;
    crate::config::write_json(&path, &settings)?;
    Ok(true)
}
