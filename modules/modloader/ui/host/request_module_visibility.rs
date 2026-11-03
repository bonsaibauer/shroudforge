use super::read_host_arguments::Arguments;
use super::read_ui_configuration::read_central_config;
use super::report_command_result::{missing_updater_message, report_command_result};
use std::process::Child;
use std::time::Instant;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

pub(crate) fn request_module_visibility(
    module: String,
    visible: bool,
    request_id: String,
    arguments: &Arguments,
    webview: &wry::WebView,
    desktop_debug_console: &mut Option<Child>,
    next_refresh: &mut Instant,
) {
    let config = read_central_config(&arguments.root);
    let configuration_error = config["configurationError"].as_str();
    let module_enabled = config["modules"][module.as_str()]["enabled"]
        .as_bool()
        .unwrap_or(false);
    let updater_missing = module == "updates"
        && visible
        && module_enabled
        && !shroudforge_loader_config::paths::updater_available(&arguments.root);
    let result = if visible && configuration_error.is_some() {
        Err(format!(
            "Could not read ShroudForge module settings: {}",
            configuration_error.unwrap_or_default()
        ))
    } else if updater_missing {
        Err(missing_updater_message(&arguments.root))
    } else if visible
        && matches!(
            module.as_str(),
            "debugConsole" | "modloaderUi" | "worldEditor" | "updates"
        )
        && !module_enabled
    {
        if module == "updates" {
            let _ = shroudforge_loader_config::config::request_window_visibility(
                &arguments.root,
                "updates",
                false,
            );
        }
        Err(format!(
            "{module} module is disabled in Settings. Its service is unavailable until it is enabled."
        ))
    } else if module == "updates" && visible {
        shroudforge_loader_config::config::request_window_visibility(&arguments.root, &module, true)
            .and_then(|()| start_updater_window(&arguments.root))
    } else if module == "debugConsole" && visible && arguments.desktop {
        if desktop_debug_console.is_none() {
            let executable = shroudforge_loader_config::paths::loader_executable(&arguments.root);
            match std::process::Command::new(executable)
                .args(["--debug-console", "--desktop", "--root"])
                .arg(&arguments.root)
                .arg("--owner-pid")
                .arg(unsafe { GetCurrentProcessId() }.to_string())
                .spawn()
            {
                Ok(child) => {
                    *desktop_debug_console = Some(child);
                    Ok(())
                }
                Err(error) => Err(format!("Could not start Desktop Debug Console: {error}")),
            }
        } else {
            shroudforge_loader_config::config::request_window_visibility(
                &arguments.root,
                &module,
                true,
            )
        }
    } else {
        shroudforge_loader_config::config::request_window_visibility(
            &arguments.root,
            &module,
            visible,
        )
    };
    let action = if visible {
        "Show window"
    } else {
        "Hide window"
    };
    report_command_result(
        &arguments.root,
        &webview,
        &request_id,
        &module,
        action,
        result,
        "settings.visibilityRequested",
        None,
        false,
    );
    *next_refresh = Instant::now();
}

fn start_updater_window(root: &std::path::Path) -> Result<(), String> {
    use std::process::Stdio;

    let config = shroudforge_loader_config::config::read_loader(root)?;
    if !config
        .pointer("/modules/updates/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        let _ =
            shroudforge_loader_config::config::request_window_visibility(root, "updates", false);
        return Err("Updates module is disabled in Settings. The update service is unavailable until it is enabled.".into());
    }
    let executable = shroudforge_loader_config::paths::updater_executable(root);
    if !executable.is_file() {
        return Err(missing_updater_message(root));
    }
    let mut command = std::process::Command::new(&executable);
    command
        .arg("--open-window")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    match command.spawn() {
        Ok(_) => Ok(()),
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                return Err(missing_updater_message(root));
            }
            let _ = shroudforge_loader_config::config::request_window_visibility(
                root, "updates", false,
            );
            Err(format!("could not open the updater window: {error}"))
        }
    }
}
