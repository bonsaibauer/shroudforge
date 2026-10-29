use super::loader_settings_input::UiSettings;
use super::record_ui_log::UiLogEvent;
use shroudforge_package::HotkeyAssessmentTarget;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiCommand {
    #[serde(default)]
    pub(crate) revision: Option<String>,
    pub(crate) command: String,
    #[serde(default)]
    pub(crate) settings: Option<UiSettings>,
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) mod_id: Option<String>,
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) values: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) locale: Option<String>,
    #[serde(default)]
    pub(crate) action: Option<String>,
    #[serde(default)]
    pub(crate) job_id: Option<String>,
    #[serde(default)]
    pub(crate) enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) device: Option<shroudforge_hotkeys::InputDevice>,
    #[serde(default)]
    pub(crate) ids: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) selected: Option<bool>,
    #[serde(default)]
    pub(crate) remove_mods: Option<bool>,
    #[serde(default)]
    pub(crate) restore_gamefiles: Option<bool>,
    #[serde(default)]
    pub(crate) reset_user_settings: Option<bool>,
    #[serde(default)]
    pub(crate) purge_data: Option<bool>,
    #[serde(default)]
    pub(crate) read: Option<bool>,
    #[serde(default)]
    pub(crate) project_id: Option<String>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) event: Option<UiLogEvent>,
    #[serde(default)]
    pub(crate) scope: Option<String>,
    #[serde(default)]
    pub(crate) module: Option<String>,
    #[serde(default)]
    pub(crate) geometry_part: Option<String>,
    #[serde(default)]
    pub(crate) key: Option<String>,
    #[serde(default)]
    pub(crate) value: serde_json::Value,
    #[serde(default)]
    pub(crate) request_id: Option<String>,
    #[serde(default)]
    pub(crate) visible: Option<bool>,
    #[serde(default)]
    pub(crate) wait_for_game: Option<bool>,
}

pub(crate) enum Command {
    Hide(String),
    Drag(String),
    MinimizeWindow,
    ToggleMaximizeWindow,
    ResetWindowGeometry(String, String, String),
    Refresh(Option<String>),
    CopyLogs(String),
    RefreshMod(String, String),
    ReloadSettings(String),
    OpenUrl(String, String),
    SetModEnabled(String, bool, String, String),
    SaveKeyReservation(
        String,
        String,
        shroudforge_hotkeys::InputDevice,
        bool,
        String,
        String,
    ),
    SaveModSettings(String, serde_json::Value, String, String),
    AssessHotkey(HotkeyAssessmentTarget, String),
    SaveSettings(UiSettings, String),
    SaveSetting(String, Option<String>, String, serde_json::Value, String),
    BrowsePath(String, String),
    OpenPath(String, String),
    SetWindowVisibility(String, bool, String),
    SaveLanguage(String, String),
    RunModAction(String, String, String),
    UpdaterRequest(String, String, String, serde_json::Value),
    SetNewsRead(Vec<String>, bool, String),
    Diagnostics(String, Option<String>, String),
    UiReady,
    UiError(String),
    UiLog(UiLogEvent),
}

pub(crate) fn parse_ui_command(value: UiCommand, desktop: bool, server: bool) -> Option<Command> {
    if let Some(command) = parse_updater_request(&value, desktop, server) {
        return Some(command);
    }
    Some(match value.command.as_str() {
        "diagnostics" => Command::Diagnostics(
            value.action.unwrap_or_default(),
            value.job_id,
            value.request_id.unwrap_or_default(),
        ),
        "hide" => Command::Hide(value.request_id.unwrap_or_default()),
        "drag" => Command::Drag(value.request_id.unwrap_or_default()),
        "minimize-window" => Command::MinimizeWindow,
        "toggle-maximize-window" => Command::ToggleMaximizeWindow,
        "reset-window-geometry" => match (value.module, value.geometry_part) {
            (Some(module), Some(component)) => Command::ResetWindowGeometry(
                module,
                component,
                value.request_id.unwrap_or_default(),
            ),
            _ => return None,
        },
        "refresh" => Command::Refresh(value.request_id),
        "copy-logs" => Command::CopyLogs(value.request_id.unwrap_or_default()),
        "refresh-mod" => match value.mod_id {
            Some(id) => Command::RefreshMod(id, value.request_id.unwrap_or_default()),
            None => return None,
        },
        "reload-settings" => Command::ReloadSettings(value.request_id.unwrap_or_default()),
        "save-language" => match value.locale {
            Some(locale) => Command::SaveLanguage(locale, value.request_id.unwrap_or_default()),
            None => return None,
        },
        "open-url" => match value.url {
            Some(url) => Command::OpenUrl(url, value.request_id.unwrap_or_default()),
            None => return None,
        },
        "save-mod-settings" => match (value.mod_id, value.values) {
            (Some(id), Some(values)) => Command::SaveModSettings(
                id,
                values,
                value.revision.unwrap_or_default(),
                value.request_id.unwrap_or_default(),
            ),
            _ => return None,
        },
        "save-key-reservation" => match (value.mod_id, value.key, value.device, value.enabled) {
            (Some(id), Some(key), Some(device), Some(reserved)) => Command::SaveKeyReservation(
                id,
                key,
                device,
                reserved,
                value.revision.unwrap_or_default(),
                value.request_id.unwrap_or_default(),
            ),
            _ => return None,
        },
        "assess-hotkey" => {
            let target = match value.scope.as_deref() {
                Some("module") if value.key.as_deref() == Some("toggleKey") => {
                    HotkeyAssessmentTarget::Module {
                        module: value.module?,
                        binding: value.value,
                    }
                }
                Some("mod") => HotkeyAssessmentTarget::Mod {
                    id: value.mod_id?,
                    key: value.key?,
                    values: value.values?,
                },
                _ => return None,
            };
            Command::AssessHotkey(target, value.request_id.unwrap_or_default())
        }
        "set-mod-enabled" => match (value.mod_id, value.enabled) {
            (Some(id), Some(enabled)) => Command::SetModEnabled(
                id,
                enabled,
                value.revision.unwrap_or_default(),
                value.request_id.unwrap_or_default(),
            ),
            _ => return None,
        },
        "save-settings" => match value.settings {
            Some(settings) => Command::SaveSettings(settings, value.request_id.unwrap_or_default()),
            None => return None,
        },
        "save-setting" => match (value.scope, value.key) {
            (Some(scope), Some(key)) => Command::SaveSetting(
                scope,
                value.module,
                key,
                value.value,
                value.request_id.unwrap_or_default(),
            ),
            _ => return None,
        },
        "browse-path" => match value.key {
            Some(key) => Command::BrowsePath(key, value.request_id.unwrap_or_default()),
            None => return None,
        },
        "open-path" => match value.key {
            Some(key) => Command::OpenPath(key, value.request_id.unwrap_or_default()),
            None => return None,
        },
        "set-window-visibility" => match (value.module, value.visible) {
            (Some(module), Some(visible)) => {
                Command::SetWindowVisibility(module, visible, value.request_id.unwrap_or_default())
            }
            _ => return None,
        },
        "run-mod-action" => match (value.mod_id, value.action) {
            (Some(id), Some(action)) => {
                Command::RunModAction(id, action, value.request_id.unwrap_or_default())
            }
            _ => return None,
        },
        "set-news-read" => Command::SetNewsRead(
            value.ids.unwrap_or_default(),
            value.read.unwrap_or(true),
            value.request_id.unwrap_or_default(),
        ),
        "ui-ready" => Command::UiReady,
        "ui-log" => Command::UiLog(value.event?),
        "ui-error" => Command::UiError(
            value
                .message
                .unwrap_or_else(|| "Unknown WebView error".into()),
        ),
        _ => return None,
    })
}

fn parse_updater_request(value: &UiCommand, desktop: bool, server: bool) -> Option<Command> {
    let (operation, command, subject) = match value.command.as_str() {
        "check-updates" => (
            "check-system-update",
            "check-system-update",
            "Updates".into(),
        ),
        "check-mod-updates" => (
            "check-mod-updates",
            "check-mod-updates",
            "ShroudEdit mods".into(),
        ),
        "stage-update" => (
            "stage-system-update",
            "stage-system-update",
            "Updates".into(),
        ),
        "queue-system-update" => (
            "enqueue-system-update",
            "enqueue-system-update",
            "Updates".into(),
        ),
        "queue-latest-installation" => (
            "queue-latest-installation",
            "queue-latest-installation",
            "ShroudForge installation".into(),
        ),
        "queue-uninstall" => (
            "queue-uninstall",
            "queue-uninstall",
            "ShroudForge uninstallation".into(),
        ),
        "select-update-queue-items" if value.selected.is_some() => (
            "select-update-queue-items",
            "select-update-queue-items",
            "Updates".into(),
        ),
        "remove-update-queue-item" => (
            "remove-update-queue-item",
            "remove-update-queue-item",
            value.key.clone()?,
        ),
        "start-update-queue" => ("start-update-queue", "start-update-queue", "Updates".into()),
        "clear-update-queue" => ("clear-update-queue", "clear-update-queue", "Updates".into()),
        "cancel-update" => ("cancel-update", "cancel-update", "Updates".into()),
        "restore-gamefiles" => (
            "restore-gamefiles",
            "restore-gamefiles",
            "Backup Management".into(),
        ),
        "capture-gamefile-originals" => (
            "capture-verified-gamefile-originals",
            "capture-verified-gamefile-originals",
            "Backup Management".into(),
        ),
        "search-catalog" => ("search-catalog", "search-catalog", value.query.clone()?),
        "install-mod" => ("install-mod", "install-mod", value.project_id.clone()?),
        "update-mod" => ("update-mod", "update-mod", value.mod_id.clone()?),
        "remove-mod" => ("remove-mod", "recycle-mod", value.mod_id.clone()?),
        "updater-request" => {
            let action = value.action.as_deref()?;
            if !matches!(
                action,
                "recycle-bin-list"
                    | "recycle-bin-restore"
                    | "recycle-bin-delete"
                    | "recycle-bin-empty"
            ) || (matches!(action, "recycle-bin-restore" | "recycle-bin-delete")
                && value.id.is_none())
            {
                return None;
            }
            (action, action, value.id.clone().unwrap_or_default())
        }
        _ => return None,
    };
    let mut request = serde_json::to_value(value).ok()?;
    request["command"] = command.into();
    match value.command.as_str() {
        "check-updates" | "check-mod-updates" => request["manual"] = true.into(),
        "stage-update" => request["waitForGame"] = value.wait_for_game.unwrap_or(!desktop).into(),
        "start-update-queue" => {
            request["waitForGame"] = value.wait_for_game.unwrap_or(!desktop).into();
            request["showWindow"] = (!server).into();
        }
        "remove-update-queue-item" => request["id"] = value.key.clone()?.into(),
        "capture-gamefile-originals" => {
            request["target"] = if server { "server" } else { "client" }.into();
        }
        _ => {}
    }
    Some(Command::UpdaterRequest(
        operation.into(),
        subject,
        value.request_id.clone().unwrap_or_default(),
        request,
    ))
}
