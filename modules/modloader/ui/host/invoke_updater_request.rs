use super::report_command_result::missing_updater_message;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
};

pub(crate) struct UpdaterRequestResult {
    pub(crate) request_id: String,
    pub(crate) operation: String,
    pub(crate) subject: String,
    pub(crate) result: Result<serde_json::Value, String>,
}

pub(crate) fn spawn(
    root: PathBuf,
    operation: String,
    subject: String,
    request_id: String,
    request: serde_json::Value,
    sender: &mpsc::Sender<UpdaterRequestResult>,
) {
    let sender = sender.clone();
    std::thread::spawn(move || {
        let result = invoke(&root, &request);
        let _ = sender.send(UpdaterRequestResult {
            request_id,
            operation,
            subject,
            result,
        });
    });
}

fn invoke(root: &Path, request: &serde_json::Value) -> Result<serde_json::Value, String> {
    let config = shroudforge_loader_config::config::read_loader(root)?;
    let updates_enabled = config
        .pointer("/modules/updates/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !updates_enabled
        && !matches!(
            request["command"].as_str(),
            Some("queue-latest-installation" | "queue-uninstall" | "start-update-queue")
        )
    {
        return Err("Updates module is disabled in Settings. The update service is unavailable until it is enabled.".into());
    }
    let executable = shroudforge_loader_config::paths::updater_executable(root);
    if !executable.is_file() {
        return Err(missing_updater_message(root));
    }
    let mut command = Command::new(&executable);
    command
        .arg("--request")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let request_bytes = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(missing_updater_message(root));
        }
        Err(error) => {
            return Err(format!(
                "could not start ShroudForge updater executable {}: {error}",
                executable.display()
            ));
        }
    };
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.wait();
        return Err("updater request stdin is unavailable".into());
    };
    let write_result = stdin
        .write_all(&request_bytes)
        .map_err(|error| format!("could not send updater request: {error}"));
    drop(stdin);
    if let Err(error) = write_result {
        let _ = child.wait();
        return Err(error);
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not read updater response: {error}"))?;
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!("updater returned an invalid response: {error}. {stderr}")
    })?;
    if !output.status.success() || response["ok"] != true {
        return Err(response["error"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| "updater request failed".into()));
    }
    response
        .get("value")
        .cloned()
        .ok_or_else(|| "updater response is missing its value".into())
}
