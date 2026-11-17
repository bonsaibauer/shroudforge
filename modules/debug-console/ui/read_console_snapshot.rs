use super::read_console_arguments::Arguments;
use super::read_console_settings::Config;
use crate::read_log_tail::LogFollower;

/// The game and loader logs as the console page holds them.
#[derive(Default)]
pub(crate) struct ConsoleFollowers {
    pub(crate) game: LogFollower,
    pub(crate) loader: LogFollower,
    target: Option<&'static str>,
}

impl ConsoleFollowers {
    /// The page lost or never had the logs: resend both tails.
    pub(crate) fn reset(&mut self) {
        self.game.reset();
        self.loader.reset();
    }
}

/// The page's settings, cheap to compare, and the log text the page lacks
/// (`null` for a log without anything new).
pub(crate) fn read_console_snapshot(
    arguments: &Arguments,
    config: &Config,
    preferences: &serde_json::Value,
    minimum_level: String,
    auto_open_alert: bool,
    followers: &mut ConsoleFollowers,
) -> (serde_json::Value, serde_json::Value) {
    let selected_server = shroudforge_loader_config::config::read_loader(&arguments.root)
        .ok()
        .and_then(|value| {
            value
                .pointer("/modules/modloaderUi/remoteTarget")
                .and_then(serde_json::Value::as_str)
                .map(|target| target == "server")
        })
        .unwrap_or(false);
    let target = if selected_server { "server" } else { "client" };
    if followers.target != Some(target) {
        followers.reset();
        followers.target = Some(target);
    }
    let (game_path, loader_path) = if selected_server {
        (
            shroudforge_loader_config::paths::remote_server_game_log(&arguments.root),
            shroudforge_loader_config::paths::remote_server_loader_log(&arguments.root),
        )
    } else {
        (
            arguments.root.join("enshrouded.log"),
            shroudforge_loader_config::paths::current_log(&arguments.root),
        )
    };
    let settings = serde_json::json!({
        "paths": {
            "game": game_path.display().to_string(),
            "loader": loader_path.display().to_string(),
        },
        "connected": !arguments.desktop,
        "target": target,
        "minimumLevel": minimum_level,
        "autoOpen": auto_open_alert,
        "preferences": preferences,
    });
    let logs = serde_json::json!({
        "game": followers.game.read(&game_path, config.tail_bytes),
        "loader": followers.loader.read(&loader_path, config.tail_bytes),
    });
    (settings, logs)
}
