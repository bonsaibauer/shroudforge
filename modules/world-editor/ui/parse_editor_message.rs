use crate::panel_layout::DEFAULT_HEIGHT;

pub(crate) enum Command {
    Ready,
    SurfaceChanged(String),
    UiMessage { level: char, message: String },
    ScriptError(String),
    Hide,
    Minimize,
    OpenModSettings,
    Drag,
    Action { action: String, value: String },
    LibraryExpanded { expanded: bool, height: u32 },
}

pub(crate) fn parse_editor_message(body: &str) -> Option<Command> {
    let command = match serde_json::from_str::<serde_json::Value>(body) {
        Ok(value) => {
            let kind = value["kind"].as_str().unwrap_or_default();
            match kind {
                "ready" => Command::Ready,
                "surface" => {
                    Command::SurfaceChanged(value["value"].as_str().unwrap_or_default().to_owned())
                }
                "ui-message" => {
                    let level = match value["level"].as_str().unwrap_or("info") {
                        "trace" => 'T',
                        "debug" => 'D',
                        "warn" => 'W',
                        "error" => 'E',
                        _ => 'I',
                    };
                    Command::UiMessage {
                        level,
                        message: value["message"].as_str().unwrap_or_default().to_owned(),
                    }
                }
                "script-error" => Command::ScriptError(
                    value["value"]
                        .as_str()
                        .unwrap_or("unknown UI script error")
                        .to_owned(),
                ),
                "hide" => Command::Hide,
                "minimize" => Command::Minimize,
                "open-mod-settings" => Command::OpenModSettings,
                "drag" => Command::Drag,
                "library-expand" => Command::LibraryExpanded {
                    expanded: value["expanded"].as_bool().unwrap_or(false),
                    height: value["height"].as_u64().unwrap_or(DEFAULT_HEIGHT as u64) as u32,
                },
                "action" => {
                    let action = value["action"].as_str().unwrap_or_default().to_owned();
                    let payload = value["value"].as_str().unwrap_or_default().to_owned();
                    Command::Action {
                        action,
                        value: payload,
                    }
                }
                _ => return None,
            }
        }
        Err(_) => return None,
    };
    Some(command)
}
