use super::ui_snapshot_types::ModuleConfig;
use super::ui_snapshot_types::UserConfig;
use shroudforge_package::config_revision;
use std::path::Path;

pub(crate) fn default_toggle_key() -> String {
    shroudforge_loader_config::config::read_loader_defaults()
        .expect("embedded loader defaults must be valid")["modules"]["modloaderUi"]["toggleKey"]
        .as_str()
        .expect("module hotkey default must be an SDL binding string")
        .to_owned()
}

pub(crate) fn default_refresh() -> u64 {
    2000
}

pub(crate) fn load_module_config(root: &Path) -> Result<ModuleConfig, String> {
    let value = shroudforge_loader_config::config::read_loader(root)?;
    serde_json::from_value(value["modules"]["modloaderUi"].clone())
        .map_err(|error| error.to_string())
}

pub(crate) fn read_central_config(root: &Path) -> serde_json::Value {
    shroudforge_loader_config::config::read_loader(root).unwrap_or_else(|error| {
        eprintln!("loader configuration: {error}");
        serde_json::json!({"configurationError": error})
    })
}

pub(crate) fn load_user_config(root: &Path) -> UserConfig {
    let value = read_central_config(root);
    UserConfig {
        update_provider: value.pointer("/catalog/provider").cloned(),
        locale: value
            .pointer("/general/locale")
            .and_then(|value| value.as_str())
            .map(str::to_owned),
    }
}

pub(crate) fn effective_provider(user: &UserConfig) -> serde_json::Value {
    user.update_provider.clone().unwrap_or_else(|| {
        serde_json::json!({
            "kind": "shroudedit",
            "baseUrl": "https://api.shroudedit.com",
            "projectId": "shroudforge",
            "loader": "shroudforge"
        })
    })
}

pub(crate) fn live_settings_revision(root: &Path) -> String {
    let config = shroudforge_loader_config::config::read_loader(root).unwrap_or_default();
    config_revision(&serde_json::json!({
        "general": config["general"],
        "logging": config["logging"],
        "exports": config["exports"],
        "paths": config["paths"],
        "runtime": config["runtime"],
        "catalog": config["catalog"],
        "modules": config["modules"]
    }))
}
