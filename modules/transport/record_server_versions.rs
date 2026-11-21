//! The Dedicated Server's multiplayer version check of this player: received
//! from the server, kept in a runtime file that the Modloader's Compatibility
//! page reads, and logged when it changes. The admin's Server view already
//! shows every player's check; this lets the player see their own.

use std::fs;
use std::path::Path;

/// Keeps the check in `value` received from `peer` when it changed: written to
/// `paths::server_versions_file` with this process's id (a file of an older
/// game session is not shown), logged as a warning when it reports issues.
pub(crate) fn record_server_versions(
    root: &Path,
    peer: u64,
    value: &serde_json::Value,
    last: &mut Option<serde_json::Value>,
) {
    let check = &value["check"];
    if check["id"] != "networkVersions" || !check["clients"].is_array() {
        return;
    }
    if last.as_ref() == Some(check) {
        return;
    }
    let file = shroudforge_loader_config::paths::server_versions_file(root);
    let document = serde_json::json!({
        "pid": std::process::id(),
        "peer": peer.to_string(),
        "check": check,
    });
    let written = file
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&file, serde_json::to_vec(&document).unwrap_or_default()));
    if let Err(error) = written {
        tracing::warn!(target: "shroudforge::network", %error, "Could not keep the server's version check");
        return;
    }
    let issues = describe_issues(check);
    if issues.is_empty() {
        tracing::info!(target: "shroudforge::network", peer, "Multiplayer versions match the server's");
    } else {
        tracing::warn!(target: "shroudforge::network", peer, %issues, "Multiplayer versions differ from the server's");
    }
    *last = Some(check.clone());
}

/// Forgets the check once the game no longer joins that server.
pub(crate) fn forget_server_versions(root: &Path, last: &mut Option<serde_json::Value>) {
    if last.take().is_some() {
        let _ = fs::remove_file(shroudforge_loader_config::paths::server_versions_file(root));
    }
}

fn describe_issues(check: &serde_json::Value) -> String {
    check["clients"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|client| client["issues"].as_array().into_iter().flatten())
        .map(|issue| {
            let text = |key: &str| issue[key].as_str().unwrap_or("-").to_owned();
            match issue["code"].as_str().unwrap_or_default() {
                "loader-version" => format!(
                    "ShroudForge {} here, {} on the server",
                    text("clientVersion"),
                    text("serverVersion")
                ),
                "mod-version" => format!(
                    "{} {} here, {} on the server",
                    text("modId"),
                    text("clientVersion"),
                    text("serverVersion")
                ),
                "mod-missing-client" => format!("{} missing here", text("modId")),
                "mod-missing-server" => format!("{} missing or disabled on the server", text("modId")),
                code => code.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_check_is_kept_and_a_repeated_one_is_not_written_again() {
        let root = std::env::temp_dir().join(format!(
            "shroudforge-server-versions-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let message = serde_json::json!({
            "type": "compatibility-result",
            "check": {
                "id": "networkVersions",
                "group": "network",
                "state": "warning",
                "detail": "",
                "clients": [{
                    "steamId": "7",
                    "state": "warning",
                    "issues": [{"code": "mod-version", "modId": "example",
                        "clientVersion": "1.0.1", "serverVersion": "1.0.0"}],
                }],
            },
        });
        let mut last = None;
        record_server_versions(&root, 9, &message, &mut last);
        let file = shroudforge_loader_config::paths::server_versions_file(&root);
        let kept: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(kept["pid"], std::process::id());
        assert_eq!(kept["check"], message["check"]);
        assert_eq!(
            describe_issues(&message["check"]),
            "example 1.0.1 here, 1.0.0 on the server"
        );
        fs::remove_file(&file).unwrap();
        record_server_versions(&root, 9, &message, &mut last);
        assert!(!file.exists(), "the same check is not written again");
        forget_server_versions(&root, &mut last);
        assert!(last.is_none());
        let _ = fs::remove_dir_all(root);
    }
}
