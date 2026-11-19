use super::read_host_arguments::Arguments;
use super::report_command_result::report_command_result;
use super::save_loader_setting::save_setting_to;
use super::window::UiEvent;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tao::event_loop::EventLoopProxy;

static REMOTE_SETTING_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn apply_loader_setting(
    scope: String,
    module: Option<String>,
    key: String,
    setting: serde_json::Value,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    remote_proxy: EventLoopProxy<UiEvent>,
    next_refresh: &mut Instant,
) {
    let selected_server = shroudforge_loader_config::config::read_loader(&arguments.root)
        .ok()
        .and_then(|config| {
            config
                .pointer("/modules/modloaderUi/remoteTarget")
                .and_then(serde_json::Value::as_str)
                .map(|target| target == "server")
        })
        .unwrap_or(false);
    let local_target_setting = module.as_deref() == Some("modloaderUi") && key == "remoteTarget";
    let remote_config = (selected_server && !arguments.server && scope == "module" && !local_target_setting)
        .then(|| shroudforge_loader_config::paths::remote_server_config(&arguments.root));

    if arguments.vanilla_session && remote_config.is_some() {
        report_command_result(&arguments.root, webview, &request_id, "Server Modloader",
            "Save server settings", Err("Server settings are unavailable while the client is in Vanilla mode".into()),
            "settings.saved", Some("settings"), true);
        return;
    }

    if let Some(remote_config) = remote_config {
        let root = arguments.root.clone();
        let action = module.as_deref().map_or_else(
            || format!("{scope}.{key}"),
            |module| format!("{module}.{key}"),
        );
        let failure_request_id = request_id.clone();
        let failure_action = format!("Save {action}");
        let failure_proxy = remote_proxy.clone();
        if let Err(message) = super::window::spawn_remote_ui_worker(move || {
            let result = save_remote_setting(
                &root,
                &remote_config,
                &scope,
                module.as_deref(),
                &key,
                &setting,
                &request_id,
            );
            let _ = remote_proxy.send_event(UiEvent::RemoteCommandResult(
                request_id,
                format!("Save {action}"),
                result,
                "settings.saved",
                true,
            ));
        }) {
            let _ = failure_proxy.send_event(UiEvent::RemoteCommandResult(
                failure_request_id,
                failure_action,
                Err(message.into()),
                "settings.saved",
                true,
            ));
        }
        *next_refresh = Instant::now();
        return;
    }

    let previous_log_level = if scope == "logging" && key == "minimumLevel" {
        shroudforge_loader_config::config::read_loader(&arguments.root)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/logging/minimumLevel")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
    } else {
        None
    };
    let result = save_setting_to(
        &arguments.root,
        &scope,
        module.as_deref(),
        &key,
        &setting,
        None,
    );
    let target = module.as_deref().map_or_else(
        || format!("{scope}.{key}"),
        |module| format!("{module}.{key}"),
    );
    let action = match (previous_log_level.as_deref(), setting.as_str()) {
        (Some(previous), Some(next)) if previous != next && result.is_ok() => {
            format!("Minimum log level changed from {previous} to {next}")
        }
        _ => format!("Save {target}"),
    };
    report_command_result(
        &arguments.root,
        webview,
        &request_id,
        "Settings",
        &action,
        result,
        "settings.saved",
        Some("settings"),
        true,
    );
    *next_refresh = Instant::now();
}

fn save_remote_setting(
    root: &std::path::Path,
    remote_config: &PathBuf,
    scope: &str,
    module: Option<&str>,
    key: &str,
    setting: &serde_json::Value,
    request_id: &str,
) -> Result<(), String> {
    let _guard = REMOTE_SETTING_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let before = shroudforge_loader_config::config::read_loader_file(root, remote_config)?;
    let expected_modules = before
        .get("modules")
        .filter(|modules| modules.is_object())
        .cloned()
        .ok_or("server Modloader configuration has no modules")?;
    save_setting_to(root, scope, module, key, setting, Some(remote_config))?;
    let updated = shroudforge_loader_config::config::read_loader_file(root, remote_config)?;
    let modules = updated
        .get("modules")
        .filter(|modules| modules.is_object())
        .cloned()
        .ok_or("server Modloader configuration has no modules")?;
    let response = shroudforge_steam_networking::request_remote_ui(
        root,
        &serde_json::json!({
            "type": "config-setting",
            "requestId": request_id,
            "expectedModules": expected_modules,
            "modules": modules
        }),
    );
    match response {
        Ok(response) if response["success"] == true => Ok(()),
        Ok(response) => {
            if response["mirrorUpdated"] == false {
                restore_mirror_if_unchanged(root, remote_config, &updated, &before);
            }
            Err(response["message"]
                .as_str()
                .unwrap_or("Server rejected the settings update")
                .to_owned())
        }
        Err(error) => {
            restore_mirror_if_unchanged(root, remote_config, &updated, &before);
            Err(error)
        }
    }
}

fn restore_mirror_if_unchanged(
    root: &std::path::Path,
    path: &std::path::Path,
    candidate: &serde_json::Value,
    previous: &serde_json::Value,
) {
    let _ = shroudforge_loader_config::config::update_loader_file(root, path, |current| {
        if current == candidate {
            *current = previous.clone();
        }
        Ok(())
    });
}
