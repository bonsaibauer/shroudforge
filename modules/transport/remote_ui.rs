//! Remote Modloader UI data and multiplayer compatibility reports.
//!
//! The game remains the Steam owner. This adapter reuses the bounded Steam UI
//! channel for server snapshots, logs, settings and authenticated client reports.

use crate::MAX_MESSAGE_BYTES;
use base64::Engine as _;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_LOG_TAIL_BYTES: u64 = 64 * 1024;
const MAX_LOG_DELTA_BYTES: u64 = 64 * 1024;
const ARCHIVE_CHUNK_BYTES: usize = 256 * 1024;
const ARCHIVE_INDEX_BATCH_SIZE: usize = 128;
const RECEIVE_BUDGET: usize = 16;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const SERVER_PEER_TIMEOUT: Duration = Duration::from_secs(30);
const LOGIN_FAILURE_LIMIT: u8 = 5;
const LOGIN_COOLDOWN: Duration = Duration::from_secs(60);
const PENDING_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PENDING_REQUESTS: usize = 32;
const RECEIVE_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(30);
const COMPATIBILITY_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const CLIENT_REPORT_INTERVAL: Duration = Duration::from_secs(10);
const CLIENT_REPORT_MISSING_AFTER: Duration = Duration::from_secs(15);
const COMPLETED_ACTION_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_COMPLETED_ACTIONS: usize = 128;
const MAX_REMOTE_LOG_MIRROR_BYTES: u64 = 1024 * 1024;
const RETAIN_REMOTE_LOG_MIRROR_BYTES: u64 = 512 * 1024;
const MAX_SERVER_MOD_ICON_DATA_BYTES: usize = 192 * 1024;
const MAX_SERVER_MOD_ICON_FILE_BYTES: usize = 96 * 1024;

#[derive(Default)]
struct ClientState {
    peer: Option<u64>,
    last_heartbeat: Option<Instant>,
    admin_token: Option<String>,
    session_generation: u64,
    pending: HashMap<String, PendingRequest>,
    mirror_errors: HashSet<&'static str>,
    archive_transfers: HashMap<String, (u64, u64)>,
    last_receive_error_log: Option<Instant>,
    last_compatibility_report: Option<Instant>,
    compatibility_report: Option<serde_json::Value>,
    compatibility_report_peer: Option<u64>,
}

struct PendingRequest {
    response: Sender<serde_json::Value>,
    request_kind: String,
    expected_response: &'static str,
    session_generation: u64,
    started: Instant,
}

#[derive(Default)]
struct FileCursor {
    offset: Option<u64>,
}

#[derive(Default)]
struct ServerState {
    peers: HashMap<u64, Instant>,
    log_sync_peers: HashSet<u64>,
    admin_sessions: HashMap<u64, AdminSession>,
    login_failures: HashMap<u64, LoginFailures>,
    game: FileCursor,
    loader: FileCursor,
    config_snapshot: Option<serde_json::Value>,
    completed_actions: HashMap<(u64, String), CompletedAction>,
    completed_action_order: VecDeque<(u64, String)>,
    last_receive_error_log: Option<Instant>,
    last_peer_discovery_error_log: Option<Instant>,
    last_log_policy_refresh: Option<Instant>,
    last_compatibility_refresh: Option<Instant>,
    compatibility_peers: HashMap<u64, CompatibilityPeer>,
    compatibility_peers_available: bool,
    compatibility_mods: Vec<mod_loader::ClientModVersion>,
    compatibility_inventory_available: bool,
    compatibility_check: Option<mod_loader::MultiplayerVersionCheck>,
    last_logged_compatibility_issues: HashMap<String, String>,
    archive_transfers: HashMap<u64, VecDeque<ServerArchiveTransfer>>,
}

struct ServerArchiveTransfer {
    name: String,
    path: PathBuf,
    size: u64,
    offset: u64,
}

struct CompatibilityPeer {
    connected_since: Instant,
    report: Option<mod_loader::ClientCompatibilityReport>,
}

struct CompletedAction {
    mod_id: String,
    action: String,
    response: serde_json::Value,
    completed: Instant,
}

struct AdminSession {
    token: String,
    group_name: String,
}

#[derive(Default)]
struct LoginFailures {
    count: u8,
    blocked_until: Option<Instant>,
    last_attempt: Option<Instant>,
}

#[derive(Default)]
struct State {
    client: ClientState,
    server: ServerState,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

/// Transfers the selected server's config snapshot and log tails/deltas.
/// Called from the existing in-game runtime tick after Steam health service.
pub fn tick(root: &Path, config: Option<&serde_json::Value>, is_server: bool) {
    let Some(config) = config else {
        stop_client_subscription();
        if let Some(state) = STATE.get() {
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.server.admin_sessions.clear();
            state.server.login_failures.clear();
            state.server.peers.clear();
            state.server.log_sync_peers.clear();
            state.server.game.offset = None;
            state.server.loader.offset = None;
            state.server.config_snapshot = None;
            state.server.last_receive_error_log = None;
            state.server.last_peer_discovery_error_log = None;
            state.server.last_log_policy_refresh = None;
            state.server.last_compatibility_refresh = None;
            state.server.compatibility_peers.clear();
            state.server.compatibility_peers_available = false;
            state.server.compatibility_mods.clear();
            state.server.compatibility_inventory_available = false;
            state.server.compatibility_check = None;
            state.server.last_logged_compatibility_issues.clear();
            state.server.archive_transfers.clear();
        }
        return;
    };
    if config
        .pointer("/modules/network/enabled")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        stop_client_subscription();
        if let Some(state) = STATE.get() {
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.server.admin_sessions.clear();
            state.server.login_failures.clear();
            state.server.peers.clear();
            state.server.log_sync_peers.clear();
            state.server.game.offset = None;
            state.server.loader.offset = None;
            state.server.config_snapshot = None;
            state.server.last_receive_error_log = None;
            state.server.last_peer_discovery_error_log = None;
            state.server.last_log_policy_refresh = None;
            state.server.last_compatibility_refresh = None;
            state.server.compatibility_peers.clear();
            state.server.compatibility_peers_available = false;
            state.server.compatibility_mods.clear();
            state.server.compatibility_inventory_available = false;
            state.server.compatibility_check = None;
            state.server.last_logged_compatibility_issues.clear();
            state.server.archive_transfers.clear();
        }
        return;
    }

    let state = STATE.get_or_init(|| Mutex::new(State::default()));
    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if is_server {
        server_tick(root, config, &mut state.server);
    } else {
        client_tick(root, config, &mut state.client);
    }
}

fn stop_client_subscription() {
    let Some(state) = STATE.get() else { return };
    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(peer) = state.client.peer.take() {
        let _ = send_json(peer, &serde_json::json!({ "type": "unsubscribe" }));
    }
    state.client.last_heartbeat = None;
    state.client.archive_transfers.clear();
    clear_admin_session(&mut state.client);
    for (_, pending) in state.client.pending.drain() {
        let _ = pending.response.send(serde_json::json!({
            "success": false,
            "message": "Server connection ended before the request completed",
            "mirrorUpdated": false
        }));
    }
    process_local_requests(None, &mut state.client);
}

fn client_tick(root: &Path, config: &serde_json::Value, client: &mut ClientState) {
    send_compatibility_report(root, client);
    let selected_server = config
        .pointer("/modules/modloaderUi/remoteTarget")
        .and_then(serde_json::Value::as_str)
        == Some("server");
    if !selected_server {
        if client.last_heartbeat.is_some() {
            if let Some(peer) = client.peer {
                let _ = send_json(peer, &serde_json::json!({ "type": "unsubscribe" }));
            }
        }
        client.last_heartbeat = None;
        client.archive_transfers.clear();
        fail_pending(
            client,
            "Server target was closed before the request completed",
        );
        process_local_requests(None, client);
        return;
    }
    super::local_bridge::start();
    let configured = config
        .pointer("/modules/network/serverSteamId")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0);
    let discovered = crate::status()
        .remote_dedicated_server_steam_id
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0);
    let Some(peer) = configured.or(discovered) else {
        if let Some(previous) = client.peer.take() {
            let _ = send_json(previous, &serde_json::json!({ "type": "unsubscribe" }));
        }
        client.last_heartbeat = None;
        client.archive_transfers.clear();
        clear_admin_session(client);
        fail_pending(client, "No connected Enshrouded server was found");
        process_local_requests(None, client);
        return;
    };

    if client.peer != Some(peer) {
        if let Some(previous) = client.peer.replace(peer) {
            let _ = send_json(previous, &serde_json::json!({ "type": "unsubscribe" }));
        }
        client.last_heartbeat = None;
        client.archive_transfers.clear();
        clear_admin_session(client);
        fail_pending(
            client,
            "Server connection changed before the request completed",
        );
    }
    expire_pending(client);
    if client
        .last_heartbeat
        .map_or(true, |last| last.elapsed() >= HEARTBEAT_INTERVAL)
    {
        if send_json(peer, &serde_json::json!({ "type": "subscribe" })).is_ok() {
            client.last_heartbeat = Some(Instant::now());
        }
    }

    process_local_requests(Some(peer), client);

    for _ in 0..RECEIVE_BUDGET {
        let message = match crate::receive_remote_ui() {
            Ok(Some(message)) => {
                client.last_receive_error_log = None;
                message
            }
            Ok(None) => {
                client.last_receive_error_log = None;
                break;
            }
            Err(error) => {
                if should_log_receive_error(&mut client.last_receive_error_log) {
                    tracing::warn!(target: "shroudforge::network", role = "client", %error, "Remote UI Steam receive failed");
                }
                break;
            }
        };
        if message.peer_steam_id.parse::<u64>().ok() != Some(peer) {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&message.payload) else {
            continue;
        };
        match value["type"].as_str() {
            Some("log") if client.admin_token.is_some() => receive_log(root, &value, client),
            Some("archive-index") if client.admin_token.is_some() => {
                receive_archive_index(root, &value, client)
            }
            Some("archive-chunk") if client.admin_token.is_some() => {
                receive_archive_chunk(root, &value, client)
            }
            Some("config") => {
                receive_config(root, &value["value"], client);
            }
            Some("admin-auth-result") => receive_auth_result(&value, client),
            Some("config-update-result") => receive_config_result(root, &value, client),
            Some("mods-list-result" | "mod-operation-result" | "server-rights-result") => {
                receive_mod_result(&value, client)
            }
            _ => {}
        }
    }
}

fn send_compatibility_report(root: &Path, client: &mut ClientState) {
    let peer = crate::status()
        .remote_dedicated_server_steam_id
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0);
    let Some(peer) = peer else {
        client.compatibility_report_peer = None;
        client.last_compatibility_report = None;
        client.compatibility_report = None;
        return;
    };
    let changed_peer = client.compatibility_report_peer != Some(peer);
    if changed_peer {
        client.compatibility_report = None;
    }
    if !changed_peer
        && client
            .last_compatibility_report
            .is_some_and(|last| last.elapsed() < CLIENT_REPORT_INTERVAL)
    {
        return;
    }

    client.compatibility_report_peer = Some(peer);
    client.last_compatibility_report = Some(Instant::now());
    let mods = match compatibility_mod_inventory(root) {
        Ok(mods) => mods,
        Err(error) => {
            tracing::warn!(target: "shroudforge::network", %error, "Could not read local mods for the multiplayer compatibility report");
            return;
        }
    };
    let configuration = mod_loader::compatibility_status::configuration(
        root,
        false,
        env!("CARGO_PKG_VERSION"),
        shroudforge_loader_config::runtime_status::runtime_pid(root),
    );
    let report = mod_loader::ClientCompatibilityReport {
        schema_version: 1,
        loader_version: mod_loader::installed_version(root),
        player_name: shroudforge_player_name::detect_active_character(root).name,
        mods,
        problems: mod_loader::client_problems(&configuration),
    };
    if let Err(error) = report.validate() {
        tracing::warn!(target: "shroudforge::network", %error, "Skipped invalid local multiplayer compatibility report");
        return;
    }
    let payload = serde_json::json!({ "type": "compatibility-report", "report": report });
    if !changed_peer && client.compatibility_report.as_ref() == Some(&payload) {
        return;
    }
    if let Err(error) = send_json(peer, &payload) {
        tracing::debug!(target: "shroudforge::network", peer, %error, "Could not send multiplayer compatibility report");
    } else {
        client.compatibility_report = Some(payload);
    }
}

fn compatibility_mod_inventory(root: &Path) -> Result<Vec<mod_loader::ClientModVersion>, String> {
    let entries = match fs::read_dir(shroudforge_loader_config::paths::mods_dir(root)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Could not read installed mods: {error}")),
    };
    let mut mods = Vec::new();
    for entry in entries.flatten() {
        let Ok(manifest) = mod_loader::read_manifest_path(root, &entry.path()) else {
            continue;
        };
        mods.push(mod_loader::ClientModVersion {
            id: manifest.id,
            version: manifest.version.to_string(),
            enabled: manifest.enabled,
            target: manifest.target,
        });
    }
    Ok(mods)
}

fn fail_pending(client: &mut ClientState, message: &str) {
    let failed_count = client.pending.len();
    for (_, pending) in client.pending.drain() {
        let _ = pending.response.send(serde_json::json!({
            "success": false,
            "message": message,
            "mirrorUpdated": false
        }));
    }
    if failed_count > 0 {
        tracing::debug!(
            target: "shroudforge::network",
            count = failed_count,
            "Cleared pending remote UI requests"
        );
    }
}

fn expire_pending(client: &mut ClientState) {
    client.pending.retain(|request_id, pending| {
        if pending.started.elapsed() < PENDING_REQUEST_TIMEOUT {
            true
        } else {
            tracing::warn!(
                target: "shroudforge::network",
                request_id = %request_id,
                request_kind = %pending.request_kind,
                elapsed_ms = pending.started.elapsed().as_millis() as u64,
                "Remote UI request timed out. Check current server state before retrying"
            );
            let _ = pending.response.send(serde_json::json!({
                "success": false,
                "message": "Server response timed out; refresh the server state before retrying"
            }));
            false
        }
    });
}

fn process_local_requests(peer: Option<u64>, client: &mut ClientState) {
    for request in super::local_bridge::drain() {
        let value = request.value;
        let request_id = value["requestId"].as_str().unwrap_or_default().to_owned();
        let request_type = value["type"].as_str().unwrap_or("unknown");
        let (expected_response, request_kind) = match request_type {
            "admin-login" => ("admin-auth-result", "admin-login".to_owned()),
            "config-setting" => ("config-update-result", "config-setting".to_owned()),
            "mods-list" => ("mods-list-result", "mods-list".to_owned()),
            "server-rights-list" | "server-rights-update" => (
                "server-rights-result",
                request_type.to_owned(),
            ),
            "mod-operation" => (
                "mod-operation-result",
                format!(
                    "mod-operation/{}",
                    value["operation"].as_str().unwrap_or("unknown")
                ),
            ),
            _ => ("unknown", request_type.to_owned()),
        };
        let Some(peer) = peer else {
            let _ = request.response.send(serde_json::json!({
                "success": false,
                "message": "No connected Enshrouded server was found",
                "mirrorUpdated": false
            }));
            continue;
        };
        if client.pending.len() >= MAX_PENDING_REQUESTS {
            let _ = request.response.send(serde_json::json!({
                "success": false,
                "message": "Too many server requests are still pending",
                "mirrorUpdated": false
            }));
            continue;
        }
        if client.pending.contains_key(&request_id) {
            let _ = request.response.send(serde_json::json!({
                "success": false,
                "message": "Duplicate server request identifier",
                "mirrorUpdated": false
            }));
            continue;
        }
        let payload = match value["type"].as_str() {
            Some("admin-login") => serde_json::json!({
                "type": "admin-login",
                "requestId": request_id.clone(),
                "password": value["password"]
            }),
            Some("config-setting") => {
                let Some(token) = client.admin_token.as_deref() else {
                    let _ = request.response.send(serde_json::json!({
                        "success": false,
                        "message": "Sign in with a server admin group before changing settings",
                        "mirrorUpdated": false
                    }));
                    continue;
                };
                serde_json::json!({
                    "type": "config-update",
                    "requestId": request_id.clone(),
                    "token": token,
                    "expectedModules": value["expectedModules"],
                    "modules": value["modules"]
                })
            }
            Some("mods-list")
            | Some("mod-operation")
            | Some("server-rights-list")
            | Some("server-rights-update") => {
                let Some(token) = client.admin_token.as_deref() else {
                    let _ = request.response.send(serde_json::json!({
                        "success": false,
                        "authorized": false,
                        "message": "Sign in with a server admin group before managing server mods"
                    }));
                    continue;
                };
                let mut payload = value.clone();
                payload["token"] = token.into();
                payload
            }
            _ => {
                let _ = request.response.send(serde_json::json!({
                    "success": false,
                    "message": "Unsupported server request",
                    "mirrorUpdated": false
                }));
                continue;
            }
        };
        if request_id.is_empty() || request_id.len() > 128 {
            let _ = request.response.send(serde_json::json!({
                "success": false,
                "message": "Invalid request identifier",
                "mirrorUpdated": false
            }));
            continue;
        }
        let Ok(bytes) = serde_json::to_vec(&payload) else {
            let _ = request.response.send(serde_json::json!({
                "success": false,
                "message": "Could not encode server request",
                "mirrorUpdated": false
            }));
            continue;
        };
        if let Err(error) = crate::send_remote_ui(peer, &bytes) {
            tracing::warn!(
                target: "shroudforge::network",
                request_id = %request_id,
                request_kind = %request_kind,
                %error,
                "Could not send remote UI request to server"
            );
            let _ = request.response.send(serde_json::json!({
                "success": false,
                "message": error,
                "mirrorUpdated": false
            }));
            continue;
        }
        client.pending.insert(
            request_id.clone(),
            PendingRequest {
                response: request.response,
                request_kind: request_kind.clone(),
                expected_response,
                session_generation: client.session_generation,
                started: Instant::now(),
            },
        );
        tracing::debug!(
            target: "shroudforge::network",
            request_id = %request_id,
            request_kind = %request_kind,
            "Remote UI request sent to server"
        );
    }
}

fn receive_mod_result(value: &serde_json::Value, client: &mut ClientState) {
    let Some(request_id) = value["requestId"].as_str() else {
        return;
    };
    let response_type = value["type"].as_str().unwrap_or_default();
    let Some(pending) = take_pending(client, request_id, response_type) else {
        return;
    };
    if value["authorized"] == false && pending.session_generation == client.session_generation {
        clear_admin_session(client);
    }
    tracing::debug!(
        target: "shroudforge::network",
        request_id,
        request_kind = %pending.request_kind,
        success = value["success"] == true,
        authorized = value["authorized"] != false,
        elapsed_ms = pending.started.elapsed().as_millis() as u64,
        "Remote UI request completed"
    );
    let _ = pending.response.send(value.clone());
}

fn receive_auth_result(value: &serde_json::Value, client: &mut ClientState) {
    let Some(request_id) = value["requestId"].as_str() else {
        return;
    };
    let Some(pending) = take_pending(client, request_id, "admin-auth-result") else {
        return;
    };
    if value["success"] == true {
        let Some(token) = value["token"]
            .as_str()
            .filter(|token| !token.is_empty() && token.len() <= 128)
        else {
            let _ = pending.response.send(serde_json::json!({ "success": false, "message": "Server returned an invalid admin session" }));
            return;
        };
        client.session_generation = client.session_generation.wrapping_add(1);
        client.admin_token = Some(token.to_owned());
        tracing::info!(
            target: "shroudforge::network",
            request_id,
            elapsed_ms = pending.started.elapsed().as_millis() as u64,
            "Server admin authentication completed"
        );
        let group_name = value["groupName"]
            .as_str()
            .filter(|group_name| !group_name.trim().is_empty())
            .unwrap_or("Server admin");
        let _ = pending.response.send(serde_json::json!({
            "success": true,
            "message": "Server admin access granted",
            "groupName": group_name
        }));
    } else {
        let message = value["message"]
            .as_str()
            .unwrap_or("Admin authentication failed");
        tracing::warn!(
            target: "shroudforge::network",
            request_id,
            elapsed_ms = pending.started.elapsed().as_millis() as u64,
            "Server admin authentication was rejected"
        );
        let _ = pending
            .response
            .send(serde_json::json!({ "success": false, "message": message }));
    }
}

fn receive_config_result(root: &Path, value: &serde_json::Value, client: &mut ClientState) {
    let mirror_updated = value
        .get("config")
        .is_some_and(|config| receive_config(root, config, client));
    let Some(request_id) = value["requestId"].as_str() else {
        return;
    };
    let Some(pending) = take_pending(client, request_id, "config-update-result") else {
        return;
    };
    if value["authorized"] == false && pending.session_generation == client.session_generation {
        clear_admin_session(client);
    }
    let success = value["success"] == true;
    let message = value["message"].as_str().unwrap_or(if success {
        "Server settings saved"
    } else {
        "Server settings changed elsewhere; the latest server settings were loaded"
    });
    tracing::debug!(
        target: "shroudforge::network",
        request_id,
        request_kind = %pending.request_kind,
        success,
        authorized = value["authorized"] != false,
        mirror_updated,
        elapsed_ms = pending.started.elapsed().as_millis() as u64,
        "Remote server settings update completed"
    );
    let _ = pending.response.send(serde_json::json!({
        "success": success,
        "message": message,
        "mirrorUpdated": mirror_updated
    }));
}

fn take_pending(
    client: &mut ClientState,
    request_id: &str,
    response_type: &str,
) -> Option<PendingRequest> {
    let Some(pending) = client.pending.get(request_id) else {
        return None;
    };
    if pending.expected_response != response_type {
        tracing::warn!(
            target: "shroudforge::network",
            request_id,
            expected = pending.expected_response,
            received = response_type,
            "Ignored remote UI response with mismatched type"
        );
        return None;
    }
    client.pending.remove(request_id)
}

fn clear_admin_session(client: &mut ClientState) {
    if client.admin_token.take().is_some() {
        client.session_generation = client.session_generation.wrapping_add(1);
    }
}

fn server_tick(root: &Path, config: &serde_json::Value, server: &mut ServerState) {
    let (allowed_clients, restrict_clients) = crate::read_network_allowlist(config);
    refresh_multiplayer_compatibility(root, server);
    let mut config_updated = false;
    for _ in 0..RECEIVE_BUDGET {
        let message = match crate::receive_remote_ui() {
            Ok(Some(message)) => {
                server.last_receive_error_log = None;
                message
            }
            Ok(None) => {
                server.last_receive_error_log = None;
                break;
            }
            Err(error) => {
                if should_log_receive_error(&mut server.last_receive_error_log) {
                    tracing::warn!(target: "shroudforge::network", role = "server", %error, "Remote UI Steam receive failed");
                }
                break;
            }
        };
        let Ok(peer) = message.peer_steam_id.parse::<u64>() else {
            continue;
        };
        let allowed = crate::is_authorized_server_peer(peer)
            && (!restrict_clients
                || allowed_clients
                    .iter()
                    .any(|value| value.parse::<u64>().ok() == Some(peer)));
        if !allowed {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&message.payload) else {
            continue;
        };
        if value["type"] == "compatibility-report" {
            if !message.reliable {
                continue;
            }
            receive_compatibility_report(peer, &value, server);
            continue;
        }
        match value["type"].as_str() {
            Some("subscribe") => {
                let first_subscription = !server.peers.contains_key(&peer);
                server.peers.insert(peer, Instant::now());
                if first_subscription {
                    if send_initial_snapshot(config, peer, server) {
                        server.config_snapshot = Some(config.clone());
                        tracing::debug!(
                            target: "shroudforge::network",
                            peer,
                            "Sent initial remote Modloader snapshot"
                        );
                    } else {
                        remove_failed_peers(server, [peer]);
                        continue;
                    }
                }
            }
            Some("unsubscribe") => {
                server.peers.remove(&peer);
                server.log_sync_peers.remove(&peer);
                server.archive_transfers.remove(&peer);
                server.admin_sessions.remove(&peer);
                server.login_failures.remove(&peer);
                forget_completed_actions(server, peer);
                tracing::debug!(target: "shroudforge::network", peer, "Remote UI peer unsubscribed");
            }
            Some("archive-request") => {
                if server.log_sync_peers.contains(&peer)
                    && admin_log_sync_enabled(root, config, peer, server)
                {
                    enqueue_server_archive(root, peer, value["name"].as_str(), server);
                }
            }
            Some("admin-login") => {
                if !server.peers.contains_key(&peer) {
                    continue;
                }
                let Some(request_id) = value["requestId"]
                    .as_str()
                    .filter(|value| !value.is_empty() && value.len() <= 128)
                else {
                    continue;
                };
                let password = value["password"].as_str().unwrap_or_default();
                if password.is_empty() || password.len() > 1024 {
                    let _ = send_auth_result(
                        peer,
                        request_id,
                        None,
                        None,
                        "Admin authentication failed",
                    );
                    continue;
                }
                if server.login_failures.get(&peer).is_some_and(|failure| {
                    failure
                        .blocked_until
                        .is_some_and(|until| until > Instant::now())
                }) {
                    let _ = send_auth_result(
                        peer,
                        request_id,
                        None,
                        None,
                        "Admin authentication failed",
                    );
                    continue;
                }
                if let Some(group_name) = matching_admin_group(root, password) {
                    server.login_failures.remove(&peer);
                    let token = uuid::Uuid::new_v4().to_string();
                    server.admin_sessions.insert(
                        peer,
                        AdminSession {
                            token: token.clone(),
                            group_name: group_name.clone(),
                        },
                    );
                    if let Err(error) =
                        send_auth_result(peer, request_id, Some(&token), Some(&group_name), "")
                    {
                        server.admin_sessions.remove(&peer);
                        tracing::warn!(
                            target: "shroudforge::network",
                            request_id,
                            %error,
                            "Could not return server admin session token"
                        );
                    } else {
                        if group_log_sync_enabled(root, config, &group_name) {
                            let reset_cursors = server.log_sync_peers.is_empty();
                            server.log_sync_peers.insert(peer);
                            if !send_initial_logs(root, peer, server, reset_cursors)
                                || !send_archive_index(root, peer)
                            {
                                remove_failed_peers(server, [peer]);
                                continue;
                            }
                        } else {
                            server.log_sync_peers.remove(&peer);
                            server.archive_transfers.remove(&peer);
                        }
                        tracing::info!(
                            target: "shroudforge::network",
                            request_id,
                            group = %group_name,
                            "Server admin session granted"
                        );
                    }
                } else {
                    let failures = server.login_failures.entry(peer).or_default();
                    failures.last_attempt = Some(Instant::now());
                    failures.count = failures.count.saturating_add(1);
                    if failures.count >= LOGIN_FAILURE_LIMIT {
                        failures.count = 0;
                        failures.blocked_until = Some(Instant::now() + LOGIN_COOLDOWN);
                        tracing::warn!(
                            target: "shroudforge::network",
                            request_id,
                            cooldown_seconds = LOGIN_COOLDOWN.as_secs(),
                            "Server admin login cooldown started after repeated failures"
                        );
                    } else {
                        tracing::debug!(
                            target: "shroudforge::network",
                            request_id,
                            failures_remaining = LOGIN_FAILURE_LIMIT - failures.count,
                            "Server admin login was rejected"
                        );
                    }
                    let _ = send_auth_result(
                        peer,
                        request_id,
                        None,
                        None,
                        "Admin authentication failed",
                    );
                }
            }
            Some("config-update") => {
                config_updated |= handle_config_update(root, peer, &value, server);
            }
            Some("mods-list") => handle_mods_list(root, peer, &value, server),
            Some("server-rights-list") => handle_server_rights_list(root, peer, &value, server),
            Some("server-rights-update") => {
                handle_server_rights_update(root, peer, &value, config, server)
            }
            Some("mod-operation") => handle_mod_operation(root, peer, &value, server),
            _ => {}
        }
    }

    let expired_peers = server
        .peers
        .iter()
        .filter_map(|(peer, last_seen)| {
            let allowed = crate::is_authorized_server_peer(*peer)
                && (!restrict_clients
                    || allowed_clients
                        .iter()
                        .any(|value| value.parse::<u64>().ok() == Some(*peer)));
            (last_seen.elapsed() >= SERVER_PEER_TIMEOUT || !allowed).then_some(*peer)
        })
        .collect::<Vec<_>>();
    for peer in expired_peers {
        server.peers.remove(&peer);
        server.log_sync_peers.remove(&peer);
        server.archive_transfers.remove(&peer);
        server.login_failures.remove(&peer);
        forget_completed_actions(server, peer);
        tracing::info!(target: "shroudforge::network", peer, "Remote UI peer disconnected or lost network authorization");
    }
    server.admin_sessions.retain(|peer, _| {
        crate::is_authorized_server_peer(*peer)
            && (!restrict_clients
                || allowed_clients
                    .iter()
                    .any(|value| value.parse::<u64>().ok() == Some(*peer)))
    });
    let refresh_log_policy = server
        .last_log_policy_refresh
        .is_none_or(|last| last.elapsed() >= COMPATIBILITY_REFRESH_INTERVAL);
    if refresh_log_policy {
        let policy_config = if server.last_log_policy_refresh.is_none() {
            shroudforge_loader_config::config::read_loader(root)
                .unwrap_or_else(|_| config.clone())
        } else {
            config.clone()
        };
        server.last_log_policy_refresh = Some(Instant::now());
        let policy = read_server_log_sync_policy(root, &policy_config);
        server.admin_sessions.retain(|_, session| {
            policy
                .get(&session.group_name)
                .is_some_and(|(is_admin, _)| *is_admin)
        });
        let log_sync_sessions = server
            .admin_sessions
            .iter()
            .filter_map(|(peer, session)| {
                (server.peers.contains_key(peer)
                    && policy
                        .get(&session.group_name)
                        .is_some_and(|(_, sync_logs)| *sync_logs))
                .then_some(*peer)
            })
            .collect::<HashSet<_>>();
        let newly_enabled_log_peers = log_sync_sessions
            .difference(&server.log_sync_peers)
            .copied()
            .collect::<Vec<_>>();
        server
            .log_sync_peers
            .retain(|peer| log_sync_sessions.contains(peer));
        server
            .archive_transfers
            .retain(|peer, _| log_sync_sessions.contains(peer));
        for peer in newly_enabled_log_peers {
            let reset_cursors = server.log_sync_peers.is_empty();
            server.log_sync_peers.insert(peer);
            if !send_initial_logs(root, peer, server, reset_cursors)
                || !send_archive_index(root, peer)
            {
                remove_failed_peers(server, [peer]);
            }
        }
    } else {
        server
            .log_sync_peers
            .retain(|peer| server.peers.contains_key(peer) && server.admin_sessions.contains_key(peer));
        server
            .archive_transfers
            .retain(|peer, _| server.log_sync_peers.contains(peer));
    }
    server.login_failures.retain(|peer, failures| {
        crate::is_authorized_server_peer(*peer)
            && failures
                .last_attempt
                .is_some_and(|last| last.elapsed() < LOGIN_COOLDOWN)
    });
    refresh_multiplayer_compatibility(root, server);
    if server.peers.is_empty() {
        server.log_sync_peers.clear();
        server.game.offset = None;
        server.loader.offset = None;
        server.config_snapshot = None;
        server.archive_transfers.clear();
        return;
    }

    if !config_updated {
        send_module_changes(config, server);
    }
    if server.peers.is_empty() {
        return;
    }
    let log_peers = server
        .log_sync_peers
        .iter()
        .filter_map(|peer| server.peers.get(peer).map(|last_seen| (*peer, *last_seen)))
        .collect::<HashMap<_, _>>();
    if !log_peers.is_empty() {
        let game = server_game_log(root);
        let loader = shroudforge_loader_config::paths::current_log(root);
        let failed = broadcast_file_delta(&log_peers, &game, "game", &mut server.game);
        remove_failed_peers(server, failed);
        if server.peers.is_empty() {
            return;
        }
        let log_peers = server
            .log_sync_peers
            .iter()
            .filter_map(|peer| server.peers.get(peer).map(|last_seen| (*peer, *last_seen)))
            .collect::<HashMap<_, _>>();
        let failed = broadcast_file_delta(&log_peers, &loader, "loader", &mut server.loader);
        remove_failed_peers(server, failed);
        service_server_archive_transfers(server);
    }
}

fn should_log_receive_error(last_logged: &mut Option<Instant>) -> bool {
    if last_logged.is_none_or(|last| last.elapsed() >= RECEIVE_ERROR_LOG_INTERVAL) {
        *last_logged = Some(Instant::now());
        true
    } else {
        false
    }
}

fn receive_compatibility_report(peer: u64, value: &serde_json::Value, server: &mut ServerState) {
    let Ok(report) =
        serde_json::from_value::<mod_loader::ClientCompatibilityReport>(value["report"].clone())
    else {
        return;
    };
    if report.validate().is_err() {
        return;
    }
    if !server.compatibility_peers.contains_key(&peer) && server.compatibility_peers.len() >= 64 {
        return;
    }
    let now = Instant::now();
    let record = server
        .compatibility_peers
        .entry(peer)
        .or_insert_with(|| CompatibilityPeer {
            connected_since: now,
            report: None,
        });
    record.report = Some(report);
}

fn refresh_multiplayer_compatibility(root: &Path, server: &mut ServerState) {
    let refresh = server
        .last_compatibility_refresh
        .is_none_or(|last| last.elapsed() >= COMPATIBILITY_REFRESH_INTERVAL);
    if refresh {
        server.last_compatibility_refresh = Some(Instant::now());
        match crate::connected_peers() {
            Ok(peers) => {
                let peers = peers
                    .into_iter()
                    .filter_map(|peer| peer.parse::<u64>().ok())
                    .filter(|peer| *peer != 0)
                    .take(64)
                    .collect::<HashSet<_>>();
                server
                    .compatibility_peers
                    .retain(|peer, _| peers.contains(peer));
                for peer in peers {
                    server
                        .compatibility_peers
                        .entry(peer)
                        .or_insert_with(|| CompatibilityPeer {
                            connected_since: Instant::now(),
                            report: None,
                        });
                }
                server.compatibility_peers_available = true;
                server.last_peer_discovery_error_log = None;
            }
            Err(error) => {
                server.compatibility_peers_available = false;
                if should_log_receive_error(&mut server.last_peer_discovery_error_log) {
                    tracing::warn!(target: "shroudforge::network", %error, "Could not enumerate authenticated game clients for compatibility checks");
                }
            }
        }
        match compatibility_mod_inventory(root) {
            Ok(mods) => {
                server.compatibility_mods = mods;
                server.compatibility_inventory_available = true;
            }
            Err(error) => {
                server.compatibility_inventory_available = false;
                if should_log_receive_error(&mut server.last_peer_discovery_error_log) {
                    tracing::warn!(target: "shroudforge::network", %error, "Could not read server mods for multiplayer compatibility checks");
                }
            }
        }
    }

    let clients = server
        .compatibility_peers
        .iter()
        .map(|(steam_id, record)| {
            let state = match record.report.as_ref() {
                Some(report) => mod_loader::ClientReportState::Reported(report.clone()),
                _ if record.connected_since.elapsed() >= CLIENT_REPORT_MISSING_AFTER => {
                    mod_loader::ClientReportState::Missing
                }
                _ => mod_loader::ClientReportState::Pending,
            };
            mod_loader::ConnectedClient {
                steam_id: steam_id.to_string(),
                state,
            }
        })
        .collect();
    let mut check = mod_loader::compatibility_check(
        &mod_loader::installed_version(root),
        &server.compatibility_mods,
        clients,
        server.compatibility_peers_available && server.compatibility_inventory_available,
    );
    if !server.compatibility_inventory_available {
        check.detail = "The server mod inventory could not be read.".into();
    }
    log_compatibility_issue_transitions(root, server, &check);
    server.compatibility_check = Some(check);
}

fn log_compatibility_issue_transitions(
    root: &Path,
    server: &mut ServerState,
    check: &mod_loader::MultiplayerVersionCheck,
) {
    let mut current = HashMap::new();
    for client in &check.clients {
        let player = client.player_name.as_deref().unwrap_or(&client.steam_id);
        for issue in &client.issues {
            if client.state != "warning" {
                continue;
            }
            let key = format!(
                "{}|{}|{}|{}|{}",
                client.steam_id,
                issue.code,
                issue.mod_id.as_deref().unwrap_or_default(),
                issue.client_version.as_deref().unwrap_or_default(),
                issue.server_version.as_deref().unwrap_or_default(),
            );
            current.insert(
                key,
                compatibility_issue_message(player, &client.steam_id, issue),
            );
        }
    }
    for (key, message) in &server.last_logged_compatibility_issues {
        if !current.contains_key(key) {
            let _ = shroudforge_logging::append(
                root,
                'I',
                "compatibility",
                &format!("Multiplayer compatibility issue resolved: {message}"),
            );
        }
    }
    for (key, message) in &current {
        if server.last_logged_compatibility_issues.get(key) != Some(message) {
            let _ = shroudforge_logging::append(
                root,
                'W',
                "compatibility",
                &format!("Multiplayer compatibility warning: {message}"),
            );
        }
    }
    server.last_logged_compatibility_issues = current;
}

fn compatibility_issue_message(
    player: &str,
    steam_id: &str,
    issue: &mod_loader::ClientVersionIssue,
) -> String {
    let identity = if player == steam_id {
        steam_id.to_owned()
    } else {
        format!("{player} ({steam_id})")
    };
    match issue.code.as_str() {
        "report-missing" => format!("{identity} has not sent a compatibility report."),
        "report-stale" => format!("{identity}'s compatibility report is stale."),
        "loader-version" => format!(
            "{identity} uses Modloader {}, server uses {}.",
            issue.client_version.as_deref().unwrap_or("unknown"),
            issue.server_version.as_deref().unwrap_or("unknown"),
        ),
        "mod-version" => format!(
            "{identity} uses mod {} version {}, server uses {}.",
            issue.mod_id.as_deref().unwrap_or("unknown"),
            issue.client_version.as_deref().unwrap_or("unknown"),
            issue.server_version.as_deref().unwrap_or("unknown"),
        ),
        "mod-missing-client" => format!(
            "Active server mod {} is missing on {identity}.",
            issue.mod_id.as_deref().unwrap_or("unknown"),
        ),
        "mod-missing-server" => format!(
            "Active client mod {} is missing or disabled on the server for {identity}.",
            issue.mod_id.as_deref().unwrap_or("unknown"),
        ),
        "client-problem" => format!(
            "{identity} reports a compatibility problem: {}",
            issue.detail.as_deref().unwrap_or("details unavailable"),
        ),
        _ => format!("{identity} has a multiplayer version compatibility issue."),
    }
}

fn send_auth_result(
    peer: u64,
    request_id: &str,
    token: Option<&str>,
    group_name: Option<&str>,
    message: &str,
) -> Result<(), String> {
    let value = match token {
        Some(token) => serde_json::json!({
            "type": "admin-auth-result",
            "requestId": request_id,
            "success": true,
            "token": token,
            "groupName": group_name
        }),
        None => serde_json::json!({
            "type": "admin-auth-result",
            "requestId": request_id,
            "success": false,
            "message": message
        }),
    };
    let bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    crate::send_remote_ui(peer, &bytes)
}

fn matching_admin_group(root: &Path, password: &str) -> Option<String> {
    let bytes = fs::read(root.join("enshrouded_server.json")).ok()?;
    let config: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    config
        .get("userGroups")?
        .as_array()?
        .iter()
        .find_map(|group| {
            let name = group.get("name")?.as_str()?;
            let correct_password = group.get("password")?.as_str()? == password;
            (!name.is_empty() && correct_password && group_has_admin_permissions(group))
                .then(|| name.to_owned())
        })
}

fn group_log_sync_enabled(root: &Path, config: &serde_json::Value, group_name: &str) -> bool {
    let Ok(bytes) = fs::read(root.join("enshrouded_server.json")) else {
        return false;
    };
    let Ok(server_config) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    let Some(group) = server_config
        .get("userGroups")
        .and_then(serde_json::Value::as_array)
        .and_then(|groups| {
            groups.iter().find(|group| {
                group.get("name").and_then(serde_json::Value::as_str) == Some(group_name)
            })
        })
    else {
        return false;
    };
    if !group_has_admin_permissions(group) {
        return false;
    }
    config
        .pointer("/modules/modloaderUi/serverRights/groupLogSync")
        .and_then(|settings| settings.get(group_name))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

fn read_server_log_sync_policy(
    root: &Path,
    config: &serde_json::Value,
) -> HashMap<String, (bool, bool)> {
    let Ok(bytes) = fs::read(root.join("enshrouded_server.json")) else {
        return HashMap::new();
    };
    let Ok(server_config) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return HashMap::new();
    };
    let settings = config
        .pointer("/modules/modloaderUi/serverRights/groupLogSync")
        .and_then(serde_json::Value::as_object);
    server_config
        .get("userGroups")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| {
            let name = group.get("name")?.as_str()?.trim();
            if name.is_empty() {
                return None;
            }
            let is_admin = group_has_admin_permissions(group);
            let sync_logs = settings
                .and_then(|settings| settings.get(name))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(is_admin)
                && is_admin;
            Some((name.to_owned(), (is_admin, sync_logs)))
        })
        .collect()
}

fn admin_log_sync_enabled(
    root: &Path,
    config: &serde_json::Value,
    peer: u64,
    server: &ServerState,
) -> bool {
    server.admin_sessions.get(&peer).is_some_and(|session| {
        admin_group_still_qualified(root, &session.group_name)
            && group_log_sync_enabled(root, config, &session.group_name)
    })
}

fn handle_config_update(
    root: &Path,
    peer: u64,
    request: &serde_json::Value,
    server: &mut ServerState,
) -> bool {
    let request_id = request["requestId"].as_str().unwrap_or_default();
    if request_id.is_empty() || request_id.len() > 128 || !server.peers.contains_key(&peer) {
        return false;
    }
    if !authorize_admin_request(root, peer, request["token"].as_str(), server) {
        tracing::warn!(
            target: "shroudforge::network",
            request_id,
            "Denied remote server settings update because the admin session is not valid"
        );
        let current = shroudforge_loader_config::config::read_loader(root).ok();
        let _ = send_config_update_result(
            peer,
            request_id,
            false,
            false,
            "Server admin access expired; sign in again",
            current.as_ref(),
        );
        return false;
    }
    tracing::debug!(
        target: "shroudforge::network",
        request_id,
        "Authorized remote server settings update"
    );

    let expected = request
        .get("expectedModules")
        .filter(|value| value.is_object());
    let modules = request.get("modules").filter(|value| value.is_object());
    let (result, conflict) = match (expected, modules) {
        (Some(expected), Some(modules)) => compare_and_swap_modules(root, expected, modules),
        _ => (Err("Invalid server settings update".into()), false),
    };
    let success = result.is_ok();
    let message = if success {
        "Server settings saved"
    } else if conflict {
        "Server settings changed in another session; the latest settings have been loaded"
    } else {
        "Server rejected the settings update"
    };
    let current = shroudforge_loader_config::config::read_loader(root).ok();
    let _ = send_config_update_result(peer, request_id, success, true, message, current.as_ref());
    if success {
        tracing::info!(target: "shroudforge::network", request_id, "Remote server settings update committed");
    } else if conflict {
        tracing::warn!(target: "shroudforge::network", request_id, "Remote server settings update rejected because another client changed the same configuration");
    } else {
        tracing::warn!(target: "shroudforge::network", request_id, "Remote server settings update failed validation or persistence");
    }
    if success {
        server.config_snapshot = None;
    }
    success
}

fn authorize_admin_request(
    root: &Path,
    peer: u64,
    supplied_token: Option<&str>,
    server: &mut ServerState,
) -> bool {
    if !server.peers.contains_key(&peer) {
        return false;
    }
    let Some(session) = server.admin_sessions.get(&peer) else {
        return false;
    };
    let token_matches = supplied_token == Some(session.token.as_str());
    let group_name = session.group_name.clone();
    if !token_matches {
        return false;
    }
    if !admin_group_still_qualified(root, &group_name) {
        server.admin_sessions.remove(&peer);
        tracing::warn!(
            target: "shroudforge::network",
            group = %group_name,
            "Server admin session revoked after its group lost a required permission"
        );
        return false;
    }
    true
}

fn handle_mods_list(root: &Path, peer: u64, request: &serde_json::Value, server: &mut ServerState) {
    let started = Instant::now();
    let Some(request_id) = request["requestId"]
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 128)
    else {
        return;
    };
    if !authorize_admin_request(root, peer, request["token"].as_str(), server) {
        tracing::warn!(
            target: "shroudforge::network",
            request_id,
            "Denied remote server mod list because the admin session is not valid"
        );
        let _ = send_mod_result(
            peer,
            "mods-list-result",
            request_id,
            false,
            false,
            "Server admin access expired; sign in again",
            None,
            None,
        );
        return;
    }
    let locale = request["locale"].as_str().unwrap_or("en-GB");
    if locale.is_empty() || locale.len() > 32 {
        let _ = send_mod_result(
            peer,
            "mods-list-result",
            request_id,
            false,
            true,
            "Invalid locale",
            None,
            None,
        );
        return;
    }
    match server_mods_snapshot(root, locale)
        .and_then(|mods| {
            server_view_snapshot(root, mods, server.compatibility_check.clone(), server)
        })
    {
        Ok((mods, server_view)) => {
            let mod_count = mods.as_array().map_or(0, Vec::len);
            let result = send_json(
                peer,
                &serde_json::json!({
                    "type": "mods-list-result",
                    "requestId": request_id,
                    "success": true,
                    "authorized": true,
                    "mods": mods,
                    "serverView": server_view
                }),
            );
            if let Err(error) = result {
                tracing::warn!(
                    target: "shroudforge::network",
                    request_id,
                    mod_count,
                    %error,
                    "Could not send server mod snapshot"
                );
                let _ = send_mod_result(
                    peer,
                    "mods-list-result",
                    request_id,
                    false,
                    true,
                    &error,
                    None,
                    None,
                );
            } else {
                tracing::debug!(
                    target: "shroudforge::network",
                    request_id,
                    mod_count,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "Server mod snapshot sent"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                target: "shroudforge::network",
                request_id,
                %error,
                "Could not read server mod snapshot"
            );
            let _ = send_mod_result(
                peer,
                "mods-list-result",
                request_id,
                false,
                true,
                &error,
                None,
                None,
            );
        }
    }
}

fn handle_server_rights_list(
    root: &Path,
    peer: u64,
    request: &serde_json::Value,
    server: &mut ServerState,
) {
    let Some(request_id) = request["requestId"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
    else {
        return;
    };
    if !authorize_admin_request(root, peer, request["token"].as_str(), server) {
        let _ = send_server_rights_result(
            peer,
            request_id,
            false,
            false,
            "Server admin access expired; sign in again",
            None,
        );
        return;
    }
    let config = shroudforge_loader_config::config::read_loader(root).unwrap_or_default();
    let result = server_rights_snapshot(root, &config, server);
    match result {
        Ok(rights) => {
            let _ = send_server_rights_result(
                peer,
                request_id,
                true,
                true,
                "Server rights loaded",
                Some(rights),
            );
        }
        Err(error) => {
            let _ = send_server_rights_result(peer, request_id, false, true, &error, None);
        }
    }
}

fn handle_server_rights_update(
    root: &Path,
    peer: u64,
    request: &serde_json::Value,
    config: &serde_json::Value,
    server: &mut ServerState,
) {
    let Some(request_id) = request["requestId"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
    else {
        return;
    };
    if !authorize_admin_request(root, peer, request["token"].as_str(), server) {
        let _ = send_server_rights_result(
            peer,
            request_id,
            false,
            false,
            "Server admin access expired; sign in again",
            None,
        );
        return;
    }
    match save_server_rights(root, &request["values"]) {
        Ok(()) => {
            server.last_log_policy_refresh = None;
            let current = shroudforge_loader_config::config::read_loader(root)
                .unwrap_or_else(|_| config.clone());
            match server_rights_snapshot(root, &current, server) {
                Ok(rights) => {
                    let _ = send_server_rights_result(
                        peer,
                        request_id,
                        true,
                        true,
                        "Server permissions saved",
                        Some(rights),
                    );
                }
                Err(error) => {
                    let _ = send_server_rights_result(
                        peer,
                        request_id,
                        false,
                        true,
                        &error,
                        None,
                    );
                }
            }
        }
        Err(error) => {
            let _ = send_server_rights_result(
                peer,
                request_id,
                false,
                true,
                &error,
                None,
            );
        }
    }
}

fn send_server_rights_result(
    peer: u64,
    request_id: &str,
    success: bool,
    authorized: bool,
    message: &str,
    server_rights: Option<serde_json::Value>,
) -> Result<(), String> {
    send_json(
        peer,
        &serde_json::json!({
            "type": "server-rights-result",
            "requestId": request_id,
            "success": success,
            "authorized": authorized,
            "message": message,
            "serverRights": server_rights
        }),
    )
}

fn handle_mod_operation(
    root: &Path,
    peer: u64,
    request: &serde_json::Value,
    server: &mut ServerState,
) {
    let Some(request_id) = request["requestId"]
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 128)
    else {
        return;
    };
    if !authorize_admin_request(root, peer, request["token"].as_str(), server) {
        tracing::warn!(
            target: "shroudforge::network",
            request_id,
            operation = request["operation"].as_str().unwrap_or("unknown"),
            "Denied remote server mod operation because the admin session is not valid"
        );
        let _ = send_mod_result(
            peer,
            "mod-operation-result",
            request_id,
            false,
            false,
            "Server admin access expired; sign in again",
            None,
            None,
        );
        return;
    }
    let Some(id) = request["modId"]
        .as_str()
        .filter(|id| mod_loader::package_lookup::valid_identifier(id))
    else {
        let _ = send_mod_result(
            peer,
            "mod-operation-result",
            request_id,
            false,
            true,
            "Invalid mod identifier",
            None,
            None,
        );
        return;
    };
    let locale = request["locale"].as_str().unwrap_or("en-GB");
    if locale.is_empty() || locale.len() > 32 {
        let _ = send_mod_result(
            peer,
            "mod-operation-result",
            request_id,
            false,
            true,
            "Invalid locale",
            None,
            None,
        );
        return;
    }
    let operation = request["operation"].as_str().unwrap_or_default();
    let valid_operation = match operation {
        "save-settings" => {
            request["values"].is_object()
                && request["revision"]
                    .as_str()
                    .is_some_and(|revision| !revision.is_empty() && revision.len() <= 128)
        }
        "set-enabled" => {
            request["enabled"].is_boolean()
                && request["revision"]
                    .as_str()
                    .is_some_and(|revision| !revision.is_empty() && revision.len() <= 128)
        }
        "key-reservation" => {
            request["key"]
                .as_str()
                .is_some_and(|key| !key.is_empty() && key.len() <= 128)
                && matches!(request["device"].as_str(), Some("keyboard" | "mouse"))
                && request["reserved"].is_boolean()
        }
        "run-action" => request["action"]
            .as_str()
            .is_some_and(mod_loader::package_lookup::valid_identifier),
        "assess-hotkey" => {
            request["key"]
                .as_str()
                .is_some_and(|key| !key.is_empty() && key.len() <= 128)
                && request["values"].is_object()
        }
        _ => false,
    };
    if !valid_operation {
        let _ = send_mod_result(
            peer,
            "mod-operation-result",
            request_id,
            false,
            true,
            "Invalid server mod operation",
            None,
            None,
        );
        return;
    }
    if operation == "run-action" {
        if let Some(action) = request["action"].as_str() {
            match cached_action_response(server, peer, request_id, id, action) {
                Ok(Some(response)) => {
                    tracing::debug!(
                        target: "shroudforge::network",
                        request_id,
                        mod_id = id,
                        operation,
                        "Replayed completed server mod action result without running it again"
                    );
                    let _ = send_json(peer, &response);
                    return;
                }
                Err(()) => {
                    let _ = send_mod_result(
                        peer,
                        "mod-operation-result",
                        request_id,
                        false,
                        true,
                        "Request identifier was already used for a different server mod action",
                        None,
                        None,
                    );
                    return;
                }
                Ok(None) => {}
            }
        }
    }
    tracing::debug!(
        target: "shroudforge::network",
        request_id,
        mod_id = id,
        operation,
        "Authorized server mod request received"
    );
    let mut assessment = None;
    let mut include_state = false;
    let result = match operation {
        "save-settings" => {
            include_state = true;
            match (request["values"].as_object(), request["revision"].as_str()) {
                (Some(_), Some(revision)) if !revision.is_empty() && revision.len() <= 128 => {
                    mod_loader::save_mod_settings(root, id, &request["values"], revision)
                }
                _ => Err("Invalid mod settings update".into()),
            }
        }
        "set-enabled" => {
            include_state = true;
            match (request["enabled"].as_bool(), request["revision"].as_str()) {
                (Some(enabled), Some(revision))
                    if !revision.is_empty() && revision.len() <= 128 =>
                {
                    mod_loader::set_mod_enabled(root, id, enabled, revision)
                }
                _ => Err("Invalid mod activation update".into()),
            }
        }
        "key-reservation" => {
            include_state = true;
            match (
                request["key"].as_str(),
                request["device"].as_str(),
                request["reserved"].as_bool(),
            ) {
                (Some(key), Some(device), Some(reserved))
                    if !key.is_empty() && key.len() <= 128 =>
                {
                    let device = match device {
                        "keyboard" => Some(shroudforge_hotkeys::InputDevice::Keyboard),
                        "mouse" => Some(shroudforge_hotkeys::InputDevice::Mouse),
                        _ => None,
                    };
                    device.map_or_else(
                        || Err("Invalid input device".into()),
                        |device| mod_loader::save_key_reservation(root, id, key, device, reserved),
                    )
                }
                _ => Err("Invalid key reservation update".into()),
            }
        }
        "run-action" => {
            let action = request["action"].as_str().unwrap_or_default();
            let package = mod_loader::package_lookup::find_mod_package(root, id);
            match package.and_then(|package| mod_loader::read_manifest_path(root, &package).ok()) {
                Some(manifest)
                    if manifest.enabled
                        && manifest.target.supports_process(true)
                        && mod_loader::queue_mod_action(root, id, action).is_ok() =>
                {
                    Ok(())
                }
                Some(manifest) if !manifest.enabled => {
                    Err("Enable this server mod before running its action".into())
                }
                Some(manifest) if !manifest.target.supports_process(true) => {
                    Err("This mod does not support the server runtime".into())
                }
                Some(_) => Err("Mod action is invalid or could not be queued".into()),
                None => Err("Server mod package was not found".into()),
            }
        }
        "assess-hotkey" => {
            let key = request["key"].as_str().unwrap_or_default();
            let target = mod_loader::HotkeyAssessmentTarget::Mod {
                id: id.to_owned(),
                key: key.to_owned(),
                values: request["values"].clone(),
            };
            match mod_loader::assess_hotkey_change(root, &target) {
                Ok(mod_loader::HotkeyAssessment::Allowed) => {
                    assessment = Some(serde_json::json!({"allowed":true}));
                    Ok(())
                }
                Ok(mod_loader::HotkeyAssessment::Confirm(key)) => {
                    assessment = Some(serde_json::json!({"allowed":true,"confirmation":key}));
                    Ok(())
                }
                Ok(mod_loader::HotkeyAssessment::Cancel) => {
                    assessment = Some(serde_json::json!({"allowed":false,"cancelled":true}));
                    Ok(())
                }
                Err(error) => {
                    assessment = Some(serde_json::json!({"allowed":false}));
                    Err(error)
                }
            }
        }
        _ => Err("Unsupported server mod operation".into()),
    };
    let success = result.is_ok();
    let message = result
        .as_ref()
        .err()
        .map(String::as_str)
        .unwrap_or(match operation {
            "save-settings" => "Server mod settings saved",
            "set-enabled" => "Server mod activation saved",
            "key-reservation" => "Server key reservation saved",
            "run-action" => "Server mod action queued",
            "assess-hotkey" => "Hotkey assessment complete",
            _ => "Server mod operation complete",
        });
    let mod_state = include_state
        .then(|| server_mod_info(root, id, locale))
        .flatten();
    let response_value = mod_result_payload(
        "mod-operation-result",
        request_id,
        success,
        true,
        message,
        mod_state.as_ref(),
        assessment.as_ref(),
    );
    if operation == "run-action" {
        if let Some(action) = request["action"].as_str() {
            remember_completed_action(server, peer, request_id, id, action, response_value.clone());
        }
    }
    let response = send_json(peer, &response_value);
    if success && operation == "save-settings" {
        tracing::info!(
            target: "shroudforge::network",
            request_id,
            mod_id = id,
            operation,
            settings = %request["values"],
            revision = request["revision"].as_str().unwrap_or_default(),
            "Server mod settings saved"
        );
    } else {
        tracing::info!(
            target: "shroudforge::network",
            request_id,
            mod_id = id,
            operation,
            success,
            "Server mod operation completed"
        );
    }
    if let Err(error) = response {
        tracing::warn!(
            target: "shroudforge::network",
            request_id,
            mod_id = id,
            operation,
            %error,
            "Could not return server mod operation result"
        );
        let _ = send_mod_result(
            peer,
            "mod-operation-result",
            request_id,
            success,
            true,
            &format!("{message}; latest mod details could not be returned: {error}"),
            None,
            assessment.as_ref(),
        );
    }
}

fn send_mod_result(
    peer: u64,
    kind: &str,
    request_id: &str,
    success: bool,
    authorized: bool,
    message: &str,
    mod_state: Option<&serde_json::Value>,
    assessment: Option<&serde_json::Value>,
) -> Result<(), String> {
    send_json(
        peer,
        &mod_result_payload(
            kind, request_id, success, authorized, message, mod_state, assessment,
        ),
    )
}

fn mod_result_payload(
    kind: &str,
    request_id: &str,
    success: bool,
    authorized: bool,
    message: &str,
    mod_state: Option<&serde_json::Value>,
    assessment: Option<&serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "type": kind,
        "requestId": request_id,
        "success": success,
        "authorized": authorized,
        "message": message,
        "mod": mod_state,
        "assessment": assessment
    })
}

fn cached_action_response(
    server: &mut ServerState,
    peer: u64,
    request_id: &str,
    mod_id: &str,
    action: &str,
) -> Result<Option<serde_json::Value>, ()> {
    server
        .completed_actions
        .retain(|_, item| item.completed.elapsed() < COMPLETED_ACTION_TTL);
    while server
        .completed_action_order
        .front()
        .is_some_and(|key| !server.completed_actions.contains_key(key))
    {
        server.completed_action_order.pop_front();
    }
    let key = (peer, request_id.to_owned());
    let Some(completed) = server.completed_actions.get(&key) else {
        return Ok(None);
    };
    if completed.mod_id != mod_id || completed.action != action {
        return Err(());
    }
    Ok(Some(completed.response.clone()))
}

fn remember_completed_action(
    server: &mut ServerState,
    peer: u64,
    request_id: &str,
    mod_id: &str,
    action: &str,
    response: serde_json::Value,
) {
    let key = (peer, request_id.to_owned());
    if !server.completed_actions.contains_key(&key) {
        server.completed_action_order.push_back(key.clone());
    }
    server.completed_actions.insert(
        key,
        CompletedAction {
            mod_id: mod_id.to_owned(),
            action: action.to_owned(),
            response,
            completed: Instant::now(),
        },
    );
    while server.completed_actions.len() > MAX_COMPLETED_ACTIONS {
        let Some(oldest) = server.completed_action_order.pop_front() else {
            break;
        };
        server.completed_actions.remove(&oldest);
    }
}

fn send_config_update_result(
    peer: u64,
    request_id: &str,
    success: bool,
    authorized: bool,
    message: &str,
    config: Option<&serde_json::Value>,
) -> Result<(), String> {
    let value = serde_json::json!({
        "type": "config-update-result",
        "requestId": request_id,
        "success": success,
        "authorized": authorized,
        "message": message,
        "config": config
    });
    let bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    crate::send_remote_ui(peer, &bytes)
}

fn admin_group_still_qualified(root: &Path, group_name: &str) -> bool {
    let Ok(bytes) = fs::read(root.join("enshrouded_server.json")) else {
        return false;
    };
    let Ok(config) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    config
        .get("userGroups")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|groups| {
            groups.iter().any(|group| {
                group.get("name").and_then(serde_json::Value::as_str) == Some(group_name)
                    && group_has_admin_permissions(group)
            })
        })
}

fn group_has_admin_permissions(group: &serde_json::Value) -> bool {
    [
        "canKickBan",
        "canAccessInventories",
        "canEditWorld",
        "canEditBase",
        "canExtendBase",
    ]
    .iter()
    .all(|key| group.get(*key).and_then(serde_json::Value::as_bool) == Some(true))
}

const SERVER_PERMISSION_KEYS: [&str; 5] = [
    "canKickBan",
    "canAccessInventories",
    "canEditWorld",
    "canEditBase",
    "canExtendBase",
];

fn group_permissions(group: &serde_json::Value) -> serde_json::Value {
    let mut rights = serde_json::Map::new();
    for key in SERVER_PERMISSION_KEYS {
        rights.insert(
            key.into(),
            serde_json::Value::Bool(group.get(key).and_then(serde_json::Value::as_bool) == Some(true)),
        );
    }
    serde_json::Value::Object(rights)
}

fn permissions_match(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    SERVER_PERMISSION_KEYS
        .iter()
        .all(|key| left.get(*key) == right.get(*key))
}

fn server_rights_snapshot(
    root: &Path,
    config: &serde_json::Value,
    server: &ServerState,
) -> Result<serde_json::Value, String> {
    let bytes = fs::read(root.join("enshrouded_server.json"))
        .map_err(|error| format!("Could not read enshrouded_server.json: {error}"))?;
    let enshrouded: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Could not parse enshrouded_server.json: {error}"))?;
    let configured_groups = enshrouded
        .get("userGroups")
        .and_then(serde_json::Value::as_array)
        .ok_or("enshrouded_server.json has no userGroups list")?;
    let sync_settings = config
        .pointer("/modules/modloaderUi/serverRights/groupLogSync")
        .and_then(serde_json::Value::as_object);
    let groups = configured_groups
        .iter()
        .filter_map(|group| {
            let name = group.get("name")?.as_str()?.trim();
            if name.is_empty() {
                return None;
            }
            let rights = group_permissions(group);
            let is_admin = group_has_admin_permissions(group);
            Some(serde_json::json!({
                "name": name,
                "canKickBan": rights["canKickBan"],
                "canAccessInventories": rights["canAccessInventories"],
                "canEditWorld": rights["canEditWorld"],
                "canEditBase": rights["canEditBase"],
                "canExtendBase": rights["canExtendBase"],
                "isShroudForgeAdmin": is_admin,
                "syncServerLogs": sync_settings
                    .and_then(|settings| settings.get(name))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(is_admin) && is_admin
            }))
        })
        .collect::<Vec<_>>();
    let session = crate::read_session_peers(root, true);
    let mut players = server
        .compatibility_peers
        .keys()
        .map(|steam_id| {
            let game_player = session.player_records.get(steam_id);
            let permissions_known = game_player.is_some();
            let permissions = game_player
                .map(|player| player.permissions.iter().map(String::as_str).collect::<HashSet<_>>())
                .unwrap_or_default();
            let rights = serde_json::json!({
                "canKickBan": permissions.contains("CanKickBan"),
                "canAccessInventories": permissions.contains("CanAccessInventories"),
                "canEditWorld": permissions.contains("CanEditWorld"),
                "canEditBase": permissions.contains("CanEditBase"),
                "canExtendBase": permissions.contains("CanExtendBase")
            });
            let matching_groups = if permissions_known {
                configured_groups
                    .iter()
                    .filter(|group| permissions_match(&rights, &group_permissions(group)))
                    .filter_map(|group| group.get("name").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            // The compatibility report is supplied by the client and its
            // player name can be spoofed. Rights management only displays the
            // name Enshrouded wrote in this server's own log.
            let player_name = game_player.and_then(|player| player.name.as_deref());
            serde_json::json!({
                "steamId": steam_id.to_string(),
                "playerName": player_name,
                "groupName": (matching_groups.len() == 1).then(|| matching_groups[0]),
                "permissionsKnown": permissions_known,
                "canKickBan": rights["canKickBan"],
                "canAccessInventories": rights["canAccessInventories"],
                "canEditWorld": rights["canEditWorld"],
                "canEditBase": rights["canEditBase"],
                "canExtendBase": rights["canExtendBase"],
                "isShroudForgeAdmin": SERVER_PERMISSION_KEYS
                    .iter()
                    .all(|key| rights[*key] == true)
            })
        })
        .collect::<Vec<_>>();
    players.sort_by(|left, right| {
        left["playerName"]
            .as_str()
            .unwrap_or(left["steamId"].as_str().unwrap_or_default())
            .to_lowercase()
            .cmp(
                &right["playerName"]
                    .as_str()
                    .unwrap_or(right["steamId"].as_str().unwrap_or_default())
                    .to_lowercase(),
            )
    });
    Ok(serde_json::json!({ "groups": groups, "players": players }))
}

fn save_server_rights(root: &Path, values: &serde_json::Value) -> Result<(), String> {
    let expected = values
        .get("expectedGroups")
        .and_then(serde_json::Value::as_array)
        .filter(|groups| groups.len() <= 64)
        .ok_or("Missing server rights snapshot. Refresh before saving")?;
    let updates = values
        .get("groups")
        .and_then(serde_json::Value::as_array)
        .filter(|groups| groups.len() <= 64)
        .ok_or("Invalid server rights update")?;
    let mut by_name = HashMap::new();
    let mut group_sync = serde_json::Map::new();
    for update in updates {
        let name = update
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|name| {
                !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
            })
            .ok_or("Invalid server group name")?;
        if by_name.contains_key(name) {
            return Err("Duplicate server group name in rights update".into());
        }
        for key in SERVER_PERMISSION_KEYS {
            if !update.get(key).is_some_and(serde_json::Value::is_boolean) {
                return Err(format!("Missing or invalid group right {key}"));
            }
        }
        let sync_server_logs = update
            .get("syncServerLogs")
            .and_then(serde_json::Value::as_bool)
            .ok_or("Missing or invalid server log sync setting")?;
        if sync_server_logs && !group_has_admin_permissions(update) {
            return Err("Only ShroudForge Admin groups can receive server logs".into());
        }
        by_name.insert(name.to_owned(), update.clone());
        group_sync.insert(name.to_owned(), serde_json::Value::Bool(sync_server_logs));
    }

    let server_path = root.join("enshrouded_server.json");
    let bytes = fs::read(&server_path).map_err(|error| error.to_string())?;
    let original: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let mut updated = original.clone();
    let groups = updated
        .get_mut("userGroups")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("enshrouded_server.json has no userGroups list")?;
    if groups.len() != by_name.len() {
        return Err("Server groups changed. Refresh the rights list before saving".into());
    }
    if groups.len() != expected.len() {
        return Err("Server groups changed. Refresh the rights list before saving".into());
    }
    let current_loader = shroudforge_loader_config::config::read_loader(root)?;
    let current_sync = current_loader
        .pointer("/modules/modloaderUi/serverRights/groupLogSync")
        .and_then(serde_json::Value::as_object);
    let expected_by_name = expected
        .iter()
        .filter_map(|group| {
            group
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(|name| (name, group))
        })
        .collect::<HashMap<_, _>>();
    if expected_by_name.len() != expected.len() {
        return Err("Invalid or duplicate expected server group".into());
    }
    for group in groups.iter() {
        let name = group
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or("A server group has no name")?;
        let expected_group = expected_by_name
            .get(name)
            .ok_or("Server groups changed. Refresh the rights list before saving")?;
        let is_admin = group_has_admin_permissions(group);
        let current_sync_enabled = current_sync
            .and_then(|settings| settings.get(name))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(is_admin)
            && is_admin;
        if !permissions_match(&group_permissions(group), &group_permissions(expected_group))
            || current_sync_enabled
                != expected_group["syncServerLogs"]
                    .as_bool()
                    .unwrap_or(false)
        {
            return Err("Server rights changed in another session. Refresh before saving".into());
        }
    }
    let mut seen = HashSet::new();
    for group in groups.iter_mut() {
        let name = group
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or("A server group has no name")?;
        let update = by_name
            .get(name)
            .ok_or("Server groups changed. Refresh the rights list before saving")?;
        seen.insert(name.to_owned());
        for key in SERVER_PERMISSION_KEYS {
            group[key] = update[key].clone();
        }
    }
    if seen.len() != by_name.len() {
        return Err("Server groups changed. Refresh the rights list before saving".into());
    }
    if groups.iter().all(|group| !group_has_admin_permissions(group)) {
        return Err("At least one ShroudForge Admin group must keep all five permissions".into());
    }
    shroudforge_loader_config::config::write_json(&server_path, &updated)?;
    if let Err(error) = shroudforge_loader_config::config::update_loader(root, |loader| {
        loader["modules"]["modloaderUi"]["serverRights"]["groupLogSync"] =
            serde_json::Value::Object(group_sync);
        Ok(())
    }) {
        let rollback = shroudforge_loader_config::config::write_json(&server_path, &original);
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "Could not save server log-sync policy ({error}) and could not restore server permissions ({rollback_error})"
            )),
        };
    }
    Ok(())
}

/// Compare and write the modules under the installation's existing config lock.
/// A stale client therefore cannot overwrite a setting saved by another client.
fn compare_and_swap_modules(
    root: &Path,
    expected: &serde_json::Value,
    modules: &serde_json::Value,
) -> (Result<(), String>, bool) {
    let mut conflict = false;
    let result = shroudforge_loader_config::config::update_loader(root, |current| {
        let current_modules = current.get("modules").cloned().unwrap_or_default();
        let protected_server_rights = current_modules
            .pointer("/modloaderUi/serverRights")
            .cloned();
        let mut expected_modules = expected.clone();
        let mut next_modules = modules.clone();
        if let Some(server_rights) = protected_server_rights {
            preserve_server_rights(&mut expected_modules, &server_rights);
            preserve_server_rights(&mut next_modules, &server_rights);
        }
        if current_modules != expected_modules {
            conflict = true;
            return Err("Server settings changed in another session".into());
        }
        current["modules"] = next_modules;
        Ok(())
    });
    (result, conflict)
}

fn preserve_server_rights(modules: &mut serde_json::Value, server_rights: &serde_json::Value) {
    let Some(modules) = modules.as_object_mut() else {
        return;
    };
    match modules.get_mut("modloaderUi") {
        Some(ui) => {
            if let Some(ui) = ui.as_object_mut() {
                ui.insert("serverRights".into(), server_rights.clone());
            }
        }
        None => {
            modules.insert(
                "modloaderUi".into(),
                serde_json::json!({ "serverRights": server_rights }),
            );
        }
    }
}

fn server_mods_snapshot(root: &Path, locale: &str) -> Result<serde_json::Value, String> {
    let entries = fs::read_dir(shroudforge_loader_config::paths::mods_dir(root))
        .map_err(|error| format!("Could not read server mods: {error}"))?;
    let mut manifests: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let package = entry.path();
            mod_loader::read_manifest_path(root, &package)
                .ok()
                .and_then(|manifest| {
                    serde_json::to_value(mod_loader::localized_manifest(&manifest, locale))
                        .ok()
                        .map(|value| (package, value))
                })
        })
        .collect();
    let installed: HashMap<String, (String, bool)> = manifests
        .iter()
        .filter_map(|(_, manifest)| {
            Some((
                manifest["id"].as_str()?.to_owned(),
                (
                    manifest["version"].as_str()?.to_owned(),
                    manifest["enabled"].as_bool().unwrap_or(false),
                ),
            ))
        })
        .collect();

    manifests.sort_by(|left, right| {
        left.1["name"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .cmp(&right.1["name"].as_str().unwrap_or_default().to_lowercase())
    });
    let mut icon_data_remaining = MAX_SERVER_MOD_ICON_DATA_BYTES;
    let mut mods = manifests
        .into_iter()
        .map(|(package, mut manifest)| {
            let config = serde_json::json!({
                "enabled": manifest["enabled"],
                "settings": manifest["settingValues"]
            });
            let revision = mod_loader::config_revision(&config);
            let capabilities = manifest["capabilities"].as_array();
            let asset = capabilities.is_some_and(|items| items.iter().any(|item| item == "patch"));
            let runtime = capabilities.is_some_and(|items| {
                items.iter().any(|item| {
                    matches!(
                        item.as_str(),
                        Some("export" | "runtime" | "runtime-register-dll")
                    )
                })
            });
            manifest["revision"] = revision.into();
            manifest["ecosystem"] =
                mod_loader::package_lookup::mod_ecosystem_for_package(&package).into();
            manifest["asset"] = asset.into();
            manifest["runtime"] = runtime.into();
            manifest["source"] = "local".into();
            let icon_assets = read_server_mod_icon(
                &package,
                manifest["icon"].as_str(),
                &mut icon_data_remaining,
            )
            .map(|(icon, data)| {
                let mut assets = serde_json::Map::new();
                assets.insert(icon, data.into());
                serde_json::Value::Object(assets)
            })
            .unwrap_or_else(|| serde_json::json!({}));
            manifest["assets"] = icon_assets;
            let dependencies = manifest["dependencies"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|dependency| {
                    let dependency_id = dependency["id"].as_str().unwrap_or_default();
                    let requirement = dependency["version"].as_str().unwrap_or_default();
                    let optional = dependency["optional"].as_bool().unwrap_or(false);
                    let candidate = installed.get(dependency_id);
                    let status = if dependency_id == "shroudforge-api" {
                        "loader-api"
                    } else if let Some((installed_version, enabled)) = candidate {
                        match semver::VersionReq::parse(requirement)
                            .ok()
                            .zip(semver::Version::parse(installed_version).ok())
                        {
                            Some((requirement, candidate)) if !requirement.matches(&candidate) => {
                                "version-mismatch"
                            }
                            Some(_) if !*enabled && optional => "optional-disabled",
                            Some(_) if !*enabled => "disabled",
                            Some(_) => "installed",
                            None => "invalid-version",
                        }
                    } else if optional {
                        "optional-missing"
                    } else {
                        "missing"
                    };
                    serde_json::json!({
                        "id": dependency_id,
                        "version": requirement,
                        "optional": optional,
                        "installedVersion": candidate.map(|(version, _)| version),
                        "status": status
                    })
                })
                .collect::<Vec<_>>();
            manifest["dependencies"] = dependencies.into();
            if let Some(changelog) = manifest["changelog"].as_array_mut() {
                changelog.truncate(50);
            }
            manifest
        })
        .collect::<Vec<_>>();
    mods.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .cmp(&right["name"].as_str().unwrap_or_default().to_lowercase())
    });
    Ok(serde_json::Value::Array(mods))
}

fn read_server_mod_icon(
    package: &Path,
    icon: Option<&str>,
    remaining_data_bytes: &mut usize,
) -> Option<(String, String)> {
    let icon = icon?;
    if icon.contains("..") || icon.contains('\\') {
        return None;
    }
    let mime = match Path::new(icon)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    if package.is_dir()
        && fs::metadata(package.join(icon)).ok()?.len() > MAX_SERVER_MOD_ICON_FILE_BYTES as u64
    {
        return None;
    }
    let bytes = mod_loader::package_files::read_package_file(package, icon).ok()?;
    if bytes.len() > MAX_SERVER_MOD_ICON_FILE_BYTES {
        return None;
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let data = format!("data:{mime};base64,{encoded}");
    if data.len() > *remaining_data_bytes {
        return None;
    }
    *remaining_data_bytes -= data.len();
    Some((icon.to_owned(), data))
}

fn server_view_snapshot(
    root: &Path,
    mods: serde_json::Value,
    version_check: Option<mod_loader::MultiplayerVersionCheck>,
    server: &ServerState,
) -> Result<(serde_json::Value, serde_json::Value), String> {
    let entries = mods
        .as_array()
        .ok_or("server mod snapshot must be an array")?;
    let owned_notice_mods = entries
        .iter()
        .map(|item| {
            (
                item["id"].as_str().unwrap_or_default().to_owned(),
                item["name"].as_str().unwrap_or_default().to_owned(),
                item["version"].as_str().unwrap_or_default().to_owned(),
                item["ecosystem"].as_str().unwrap_or("SF").to_owned(),
                item["changelog"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let notification_mods = owned_notice_mods
        .iter()
        .map(
            |(id, name, version, ecosystem, changelog)| shroudforge_news::ModNotification {
                id,
                name,
                version,
                ecosystem,
                changelog,
            },
        )
        .collect::<Vec<_>>();
    let notices = shroudforge_news::read_notifications(root, &notification_mods);
    let news_templates = shroudforge_news::read(root)
        .ok()
        .and_then(|value| value.get("templates").cloned())
        .unwrap_or_else(|| serde_json::json!({}));
    let game_version = fs::read(shroudforge_loader_config::paths::version_file(root))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value["gameVersion"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "Automatisch erkannt".into());
    let mut compatibility = mod_loader::compatibility_status::configuration(
        root,
        true,
        env!("CARGO_PKG_VERSION"),
        shroudforge_loader_config::runtime_status::runtime_pid(root),
    );
    if let (Some(checks), Some(check)) = (compatibility["checks"].as_array_mut(), version_check) {
        if let Ok(check) = serde_json::to_value(check) {
            checks.push(check);
        }
    }
    let loader_config = shroudforge_loader_config::config::read_loader(root)?;
    let server_rights = server_rights_snapshot(root, &loader_config, server)?;
    let server_view = serde_json::json!({
        "notices": notices,
        "newsTemplates": news_templates,
        "gameVersion": game_version,
        "version": mod_loader::installed_version(root),
        "configuration": compatibility,
        "dllCandidates": mod_loader::dll_manager::discover_dll_candidates(root),
        "dllChainEntries": mod_loader::dll_manager::chain_load_entries(root),
        "hotkeyConflicts": [],
        "serverRights": server_rights,
        "mode": "SERVER"
    });
    Ok((mods, server_view))
}

fn server_mod_info(root: &Path, id: &str, locale: &str) -> Option<serde_json::Value> {
    server_mods_snapshot(root, locale)
        .ok()?
        .as_array()?
        .iter()
        .find(|item| item["id"].as_str() == Some(id))
        .cloned()
}

fn send_initial_snapshot(
    config: &serde_json::Value,
    peer: u64,
    server: &ServerState,
) -> bool {
    send_config(peer, config, server.admin_sessions.contains_key(&peer)).is_ok()
}

fn send_initial_logs(
    root: &Path,
    peer: u64,
    server: &mut ServerState,
    reset_cursors: bool,
) -> bool {
    let (game_tail, game_offset) = read_tail(&server_game_log(root));
    let game_sent = send_log(peer, "game", "replace", game_tail).is_ok();
    if game_sent && (reset_cursors || server.game.offset.is_none()) {
        server.game.offset = game_offset;
    }
    let (loader_tail, loader_offset) =
        read_tail(&shroudforge_loader_config::paths::current_log(root));
    let loader_sent = send_log(peer, "loader", "replace", loader_tail).is_ok();
    if loader_sent && (reset_cursors || server.loader.offset.is_none()) {
        server.loader.offset = loader_offset;
    }
    game_sent && loader_sent
}

fn send_archive_index(root: &Path, peer: u64) -> bool {
    let files = list_server_archives(root);
    if files.is_empty() {
        return send_json(
            peer,
            &serde_json::json!({ "type": "archive-index", "files": [] }),
        )
        .is_ok();
    }
    files.chunks(ARCHIVE_INDEX_BATCH_SIZE).all(|batch| {
        let files = batch
            .iter()
            .map(|(name, size)| serde_json::json!({ "name": name, "size": size }))
            .collect::<Vec<_>>();
        send_json(
            peer,
            &serde_json::json!({ "type": "archive-index", "files": files }),
        )
        .is_ok()
    })
}

fn list_server_archives(root: &Path) -> Vec<(String, u64)> {
    let directory = shroudforge_loader_config::paths::logs_dir(root);
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_file() {
                return None;
            }
            let name = entry.file_name().into_string().ok()?;
            if !valid_server_archive_name(&name) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            (metadata.len() > 0).then_some((name, metadata.len()))
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn valid_server_archive_name(name: &str) -> bool {
    let Some(stamp_and_suffix) = name
        .strip_prefix("shroudforge-server-")
        .and_then(|value| value.strip_suffix(".log"))
    else {
        return false;
    };
    if Path::new(name).file_name().and_then(|value| value.to_str()) != Some(name) {
        return false;
    }
    stamp_and_suffix
        .split('-')
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn enqueue_server_archive(root: &Path, peer: u64, name: Option<&str>, server: &mut ServerState) {
    let Some(name) = name.filter(|value| valid_server_archive_name(value)) else {
        return;
    };
    let path = shroudforge_loader_config::paths::logs_dir(root).join(name);
    let Ok(metadata) = fs::metadata(&path) else {
        return;
    };
    if !metadata.is_file() || metadata.len() == 0 {
        return;
    }
    let queue = server.archive_transfers.entry(peer).or_default();
    if queue.iter().any(|transfer| transfer.name == name) {
        return;
    }
    queue.push_back(ServerArchiveTransfer {
        name: name.to_owned(),
        path,
        size: metadata.len(),
        offset: 0,
    });
}

fn service_server_archive_transfers(server: &mut ServerState) {
    let peers = server.archive_transfers.keys().copied().collect::<Vec<_>>();
    let mut failed = Vec::new();
    for peer in peers {
        let Some(queue) = server.archive_transfers.get_mut(&peer) else {
            continue;
        };
        let Some(transfer) = queue.front_mut() else {
            server.archive_transfers.remove(&peer);
            continue;
        };
        let remaining = transfer.size.saturating_sub(transfer.offset);
        if remaining == 0 {
            queue.pop_front();
            continue;
        }
        let chunk_length = remaining.min(ARCHIVE_CHUNK_BYTES as u64) as usize;
        let read_result = (|| {
            let mut file = File::open(&transfer.path)?;
            file.seek(SeekFrom::Start(transfer.offset))?;
            let mut bytes = vec![0; chunk_length];
            file.read_exact(&mut bytes)?;
            Ok::<_, std::io::Error>(bytes)
        })();
        let bytes = match read_result {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    target: "shroudforge::network",
                    peer,
                    archive = %transfer.name,
                    %error,
                    "Could not read server log archive for transfer"
                );
                queue.pop_front();
                continue;
            }
        };
        let payload = encode_json(&serde_json::json!({
            "type": "archive-chunk",
            "name": transfer.name,
            "size": transfer.size,
            "offset": transfer.offset,
            "data": base64::engine::general_purpose::STANDARD.encode(&bytes)
        }));
        let Ok(payload) = payload else {
            queue.pop_front();
            continue;
        };
        if crate::send_remote_ui(peer, &payload).is_err() {
            failed.push(peer);
            continue;
        }
        transfer.offset += bytes.len() as u64;
        if transfer.offset == transfer.size {
            queue.pop_front();
        }
    }
    remove_failed_peers(server, failed);
}

fn send_module_changes(config: &serde_json::Value, server: &mut ServerState) {
    if server.config_snapshot.as_ref() == Some(config) {
        return;
    }
    let failed = server
        .peers
        .keys()
        .copied()
        .filter(|peer| {
            let value = remote_config(config, server.admin_sessions.contains_key(peer));
            send_json(*peer, &serde_json::json!({ "type": "config", "value": value })).is_err()
        })
        .collect::<Vec<_>>();
    remove_failed_peers(server, failed);
    server.config_snapshot = (!server.peers.is_empty()).then(|| config.clone());
}

fn broadcast_file_delta(
    peers: &HashMap<u64, Instant>,
    path: &Path,
    source: &str,
    cursor: &mut FileCursor,
) -> Vec<u64> {
    let Some((mode, text)) = read_delta(path, &mut cursor.offset) else {
        return Vec::new();
    };
    let payload = encode_json(&serde_json::json!({
        "type": "log",
        "source": source,
        "mode": mode,
        "text": text
    }));
    let Ok(payload) = payload else {
        return Vec::new();
    };
    send_payload_to_peers(peers, &payload)
}

fn send_log(peer: u64, source: &str, mode: &str, text: String) -> Result<(), String> {
    send_json(
        peer,
        &serde_json::json!({ "type": "log", "source": source, "mode": mode, "text": text }),
    )
}

fn receive_log(root: &Path, value: &serde_json::Value, client: &mut ClientState) {
    let (source, path) = match value["source"].as_str() {
        Some("game") => (
            "game",
            shroudforge_loader_config::paths::remote_server_game_log(root),
        ),
        Some("loader") => (
            "loader",
            shroudforge_loader_config::paths::remote_server_loader_log(root),
        ),
        _ => return,
    };
    let Some(text) = value["text"].as_str() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            log_mirror_failure(client, source, parent, &error);
            return;
        }
    }
    let result = match value["mode"].as_str() {
        Some("replace") => fs::write(&path, text).map(|_| false),
        Some("append") => append_remote_log(&path, text),
        _ => return,
    };
    match result {
        Ok(trimmed) => {
            if client.mirror_errors.remove(source) {
                tracing::info!(
                    target: "shroudforge::network",
                    source,
                    "Server log mirror writes recovered"
                );
            }
            if trimmed {
                tracing::debug!(
                    target: "shroudforge::network",
                    source,
                    retained_bytes = RETAIN_REMOTE_LOG_MIRROR_BYTES,
                    "Trimmed server log mirror to its configured size bound"
                );
            }
        }
        Err(error) => log_mirror_failure(client, source, &path, &error),
    }
}

fn receive_archive_index(root: &Path, value: &serde_json::Value, client: &mut ClientState) {
    let Some(files) = value["files"].as_array() else {
        return;
    };
    let Some(peer) = client.peer else { return };
    let directory = shroudforge_loader_config::paths::logs_dir(root);
    for entry in files.iter().take(ARCHIVE_INDEX_BATCH_SIZE) {
        let Some(name) = entry["name"]
            .as_str()
            .filter(|name| valid_server_archive_name(name))
        else {
            continue;
        };
        let Some(size) = entry["size"].as_u64().filter(|size| *size > 0) else {
            continue;
        };
        let path = directory.join(name);
        let already_present =
            fs::metadata(&path).is_ok_and(|metadata| metadata.is_file() && metadata.len() == size);
        if !already_present
            && let Err(error) = send_json(
                peer,
                &serde_json::json!({ "type": "archive-request", "name": name }),
            )
        {
            log_archive_transfer_failure(client, &path, error);
        }
    }
}

fn receive_archive_chunk(root: &Path, value: &serde_json::Value, client: &mut ClientState) {
    let Some(name) = value["name"]
        .as_str()
        .filter(|name| valid_server_archive_name(name))
    else {
        return;
    };
    let Some(size) = value["size"].as_u64().filter(|size| *size > 0) else {
        return;
    };
    let Some(offset) = value["offset"].as_u64() else {
        return;
    };
    let Some(encoded) = value["data"].as_str() else {
        return;
    };
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
        return;
    };
    if bytes.is_empty()
        || bytes.len() > ARCHIVE_CHUNK_BYTES
        || offset
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > size)
    {
        return;
    }

    let directory = shroudforge_loader_config::paths::logs_dir(root);
    let path = directory.join(name);
    let partial_path = directory.join(format!("{name}.part"));
    if let Err(error) = fs::create_dir_all(&directory) {
        log_archive_transfer_failure(client, &directory, error);
        return;
    }
    let mut file = if offset == 0 {
        let opened = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&partial_path);
        match opened {
            Ok(file) => {
                if let Err(error) = file.set_len(size) {
                    log_archive_transfer_failure(client, &partial_path, error);
                    client.archive_transfers.remove(name);
                    return;
                }
                file
            }
            Err(error) => {
                log_archive_transfer_failure(client, &partial_path, error);
                client.archive_transfers.remove(name);
                return;
            }
        }
    } else {
        if client.archive_transfers.get(name) != Some(&(size, offset)) {
            return;
        }
        match OpenOptions::new().write(true).open(&partial_path) {
            Ok(file) => file,
            Err(error) => {
                log_archive_transfer_failure(client, &partial_path, error);
                client.archive_transfers.remove(name);
                return;
            }
        }
    };
    if offset == 0 {
        client.archive_transfers.insert(name.to_owned(), (size, 0));
    }
    if let Err(error) = file
        .seek(SeekFrom::Start(offset))
        .and_then(|_| file.write_all(&bytes))
    {
        log_archive_transfer_failure(client, &partial_path, error);
        client.archive_transfers.remove(name);
        return;
    }
    let next_offset = offset + bytes.len() as u64;
    if next_offset == size {
        if let Err(error) = file.flush().and_then(|_| file.sync_all()) {
            log_archive_transfer_failure(client, &partial_path, error);
            client.archive_transfers.remove(name);
            return;
        }
        drop(file);
        match fs::rename(&partial_path, &path) {
            Ok(()) => {
                client.archive_transfers.remove(name);
                tracing::debug!(
                    target: "shroudforge::network",
                    archive = %name,
                    bytes = size,
                    "Received server session log archive"
                );
                if client.mirror_errors.remove("archive") {
                    tracing::info!(
                        target: "shroudforge::network",
                        archive = %name,
                        "Server log archive transfer recovered"
                    );
                }
            }
            Err(error) => {
                log_archive_transfer_failure(client, &path, error);
                client.archive_transfers.remove(name);
            }
        }
    } else {
        client
            .archive_transfers
            .insert(name.to_owned(), (size, next_offset));
    }
}

fn log_archive_transfer_failure(
    client: &mut ClientState,
    path: &Path,
    error: impl std::fmt::Display,
) {
    if client.mirror_errors.insert("archive") {
        tracing::warn!(
            target: "shroudforge::network",
            path = %path.display(),
            %error,
            "Could not receive server log archive"
        );
    }
}

fn append_remote_log(path: &Path, text: &str) -> std::io::Result<bool> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(text.as_bytes())?;
    drop(file);
    let length = fs::metadata(path)?.len();
    if length <= MAX_REMOTE_LOG_MIRROR_BYTES {
        return Ok(false);
    }

    let start = length.saturating_sub(RETAIN_REMOTE_LOG_MIRROR_BYTES);
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity(RETAIN_REMOTE_LOG_MIRROR_BYTES as usize);
    file.take(length - start).read_to_end(&mut bytes)?;
    let first = if start == 0 {
        0
    } else {
        bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| index + 1)
    };
    let complete = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(first, |index| index + 1);
    fs::write(path, &bytes[first..complete])?;
    Ok(true)
}

fn log_mirror_failure(
    client: &mut ClientState,
    source: &'static str,
    path: &Path,
    error: impl std::fmt::Display,
) {
    if client.mirror_errors.insert(source) {
        tracing::warn!(
            target: "shroudforge::network",
            source,
            path = %path.display(),
            %error,
            "Could not update server mirror"
        );
    }
}

fn receive_config(root: &Path, value: &serde_json::Value, client: &mut ClientState) -> bool {
    if !value.is_object()
        || !value
            .get("modules")
            .is_some_and(serde_json::Value::is_object)
    {
        return false;
    }
    let path = shroudforge_loader_config::paths::remote_server_config(root);
    let write = if path.is_file() {
        shroudforge_loader_config::config::update_loader_file(root, &path, |current| {
            *current = value.clone();
            Ok(())
        })
    } else {
        shroudforge_loader_config::config::write_json(&path, value)
    };
    if let Err(error) = write {
        log_mirror_failure(client, "config", &path, &error);
        return false;
    }
    if client.mirror_errors.remove("config") {
        tracing::info!(target: "shroudforge::network", "Server configuration mirror writes recovered");
    }
    true
}

fn remote_config(config: &serde_json::Value, include_server_rights: bool) -> serde_json::Value {
    let mut value = config.clone();
    if !include_server_rights {
        if let Some(modules) = value
            .get_mut("modules")
            .and_then(serde_json::Value::as_object_mut)
        {
            if let Some(ui) = modules
                .get_mut("modloaderUi")
                .and_then(serde_json::Value::as_object_mut)
            {
                ui.remove("serverRights");
            }
        }
    }
    value
}

fn send_config(
    peer: u64,
    config: &serde_json::Value,
    include_server_rights: bool,
) -> Result<(), String> {
    send_json(
        peer,
        &serde_json::json!({
            "type": "config",
            "value": remote_config(config, include_server_rights)
        }),
    )
}

fn read_tail(path: &Path) -> (String, Option<u64>) {
    let mut cursor = FileCursor::default();
    let text = read_delta(path, &mut cursor.offset)
        .map(|(_, text)| text)
        .unwrap_or_default();
    (text, cursor.offset)
}

fn read_delta(path: &Path, offset: &mut Option<u64>) -> Option<(&'static str, String)> {
    // Most ticks have no new log data. Avoid opening and seeking the file in
    // that common case; metadata keeps the existing one-second polling cheap.
    let observed_size = fs::metadata(path).ok()?.len();
    if *offset == Some(observed_size) {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let end = file.seek(SeekFrom::End(0)).ok()?;
    if let Some(previous) = *offset {
        if end == previous {
            return None;
        }
        if end > previous && end - previous <= MAX_LOG_DELTA_BYTES {
            file.seek(SeekFrom::Start(previous)).ok()?;
            let mut bytes = Vec::new();
            file.take(end - previous).read_to_end(&mut bytes).ok()?;
            let Some(complete) = bytes
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map(|index| index + 1)
            else {
                return None;
            };
            *offset = Some(previous + complete as u64);
            return Some((
                "append",
                String::from_utf8_lossy(&bytes[..complete]).into_owned(),
            ));
        }
    }

    let start = end.saturating_sub(MAX_LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(end - start).read_to_end(&mut bytes).ok()?;
    let first = if start == 0 {
        0
    } else if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
        newline + 1
    } else {
        *offset = Some(end);
        return Some(("replace", String::new()));
    };
    let complete = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(first, |index| index + 1);
    *offset = Some(start + complete as u64);
    Some((
        "replace",
        String::from_utf8_lossy(&bytes[first..complete]).into_owned(),
    ))
}

fn server_game_log(root: &Path) -> PathBuf {
    root.join("logs").join("enshrouded_server.log")
}

fn send_json(peer: u64, value: &serde_json::Value) -> Result<(), String> {
    let payload = encode_json(value)?;
    crate::send_remote_ui(peer, &payload)
}

fn encode_json(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err("remote UI message exceeds the Steam transport limit".into());
    }
    Ok(payload)
}

fn send_payload_to_peers(peers: &HashMap<u64, Instant>, payload: &[u8]) -> Vec<u64> {
    let mut failed = Vec::new();
    for peer in peers.keys().copied() {
        if crate::send_remote_ui(peer, payload).is_err() {
            failed.push(peer);
        }
    }
    failed
}

fn remove_failed_peers(server: &mut ServerState, failed: impl IntoIterator<Item = u64>) {
    for peer in failed {
        server.peers.remove(&peer);
        server.log_sync_peers.remove(&peer);
        server.archive_transfers.remove(&peer);
        server.login_failures.remove(&peer);
        forget_completed_actions(server, peer);
        tracing::warn!(target: "shroudforge::network", peer, "Removed remote UI peer after a Steam send failure");
    }
    if server.peers.is_empty() {
        server.game.offset = None;
        server.loader.offset = None;
        server.config_snapshot = None;
    }
}

fn forget_completed_actions(server: &mut ServerState, peer: u64) {
    server
        .completed_actions
        .retain(|(candidate, _), _| *candidate != peer);
    server
        .completed_action_order
        .retain(|(candidate, _)| *candidate != peer);
}

#[cfg(test)]
mod tests {
    use super::{
        AdminSession, ClientState, MAX_REMOTE_LOG_MIRROR_BYTES, PendingRequest,
        RETAIN_REMOTE_LOG_MIRROR_BYTES, ServerState, append_remote_log, authorize_admin_request,
        cached_action_response, compare_and_swap_modules, receive_mod_result,
        remember_completed_action, server_mods_snapshot, take_pending,
    };
    use std::fs;
    use std::sync::mpsc;
    use std::time::Instant;

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("shroudforge-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn concurrent_clients_cannot_overwrite_the_same_module_revision() {
        let root = std::env::temp_dir().join(format!(
            "shroudforge-remote-ui-cas-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let updater = shroudforge_loader_config::paths::updater_executable(&root);
        fs::create_dir_all(updater.parent().unwrap()).unwrap();
        fs::write(updater, b"").unwrap();
        shroudforge_loader_config::config::initialize_loader_config(&root).unwrap();

        let current = shroudforge_loader_config::config::read_loader(&root).unwrap();
        let expected = current["modules"].clone();
        let mut first = expected.clone();
        first["modloaderUi"]["remoteTarget"] = "server".into();
        first["debugConsole"]["autoScroll"] =
            (!expected["debugConsole"]["autoScroll"].as_bool().unwrap()).into();
        let mut second = expected.clone();
        second["modloaderUi"]["remoteTarget"] = "server".into();

        let (first_result, second_result) = std::thread::scope(|scope| {
            let first_task = scope.spawn(|| compare_and_swap_modules(&root, &expected, &first));
            let second_task = scope.spawn(|| compare_and_swap_modules(&root, &expected, &second));
            (first_task.join().unwrap(), second_task.join().unwrap())
        });

        assert_ne!(first, second);
        assert_eq!(first_result.0.is_ok(), !first_result.1);
        assert_eq!(second_result.0.is_ok(), !second_result.1);
        assert_ne!(first_result.0.is_ok(), second_result.0.is_ok());
        assert!(first_result.1 || second_result.1);

        let saved = shroudforge_loader_config::config::read_loader(&root).unwrap();
        assert!(saved["modules"] == first || saved["modules"] == second);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn server_mod_snapshot_uses_existing_manifests_and_config_revision() {
        let root =
            std::env::temp_dir().join(format!("shroudforge-remote-mods-{}", uuid::Uuid::new_v4()));
        let package = shroudforge_loader_config::paths::mods_dir(&root).join("example");
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("mod.json"),
            br#"{"id":"example","name":"Example","version":"1.2.3","capabilities":["runtime"],"dependencies":[{"id":"missing","version":"^1.0.0","optional":true}]}"#,
        )
        .unwrap();
        fs::write(
            package.join("extended.mod.json"),
            br#"{"schemaVersion":1,"enabled":true,"launcher":"SF","targets":["server"],"settings":{"enabledFeature":{"value":true,"label":"Enabled","control":"toggle"}}}"#,
        )
        .unwrap();

        let mods = server_mods_snapshot(&root, "en-GB").unwrap();
        let item = &mods[0];
        assert_eq!(mods.as_array().unwrap().len(), 1);
        assert_eq!(item["id"], "example");
        assert_eq!(item["target"], "server");
        assert_eq!(item["ecosystem"], "SF");
        assert_eq!(item["runtime"], true);
        assert_eq!(item["settingValues"]["enabledFeature"], true);
        assert_eq!(item["dependencies"][0]["status"], "optional-missing");
        assert_eq!(
            item["revision"],
            mod_loader::config_revision(&serde_json::json!({
                "enabled": true,
                "settings": {"enabledFeature": true}
            }))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_response_type_does_not_consume_a_pending_request() {
        let (response, _receiver) = mpsc::channel();
        let mut client = ClientState::default();
        client.pending.insert(
            "request-1".into(),
            PendingRequest {
                response,
                request_kind: "mod-operation/run-action".into(),
                expected_response: "mod-operation-result",
                session_generation: 0,
                started: Instant::now(),
            },
        );

        assert!(take_pending(&mut client, "request-1", "mods-list-result").is_none());
        assert!(client.pending.contains_key("request-1"));
        assert!(take_pending(&mut client, "request-1", "mod-operation-result").is_some());
    }

    #[test]
    fn late_rejection_from_an_old_session_does_not_clear_a_new_token() {
        let (response, _receiver) = mpsc::channel();
        let mut client = ClientState {
            admin_token: Some("new-token".into()),
            session_generation: 2,
            ..ClientState::default()
        };
        client.pending.insert(
            "request-1".into(),
            PendingRequest {
                response,
                request_kind: "mod-operation/save-settings".into(),
                expected_response: "mod-operation-result",
                session_generation: 1,
                started: Instant::now(),
            },
        );

        receive_mod_result(
            &serde_json::json!({
                "type": "mod-operation-result",
                "requestId": "request-1",
                "success": false,
                "authorized": false
            }),
            &mut client,
        );

        assert_eq!(client.admin_token.as_deref(), Some("new-token"));
    }

    #[test]
    fn stale_token_does_not_revoke_a_current_admin_session() {
        let root = temporary_directory("remote-admin-session");
        fs::write(
            root.join("enshrouded_server.json"),
            br#"{"userGroups":[{"name":"Admin","password":"unused","canKickBan":true,"canAccessInventories":true,"canEditWorld":true,"canEditBase":true,"canExtendBase":true}]}"#,
        )
        .unwrap();
        let mut server = ServerState::default();
        server.peers.insert(42, Instant::now());
        server.admin_sessions.insert(
            42,
            AdminSession {
                token: "current-token".into(),
                group_name: "Admin".into(),
            },
        );

        assert!(!authorize_admin_request(
            &root,
            42,
            Some("stale-token"),
            &mut server
        ));
        assert_eq!(server.admin_sessions[&42].token, "current-token");
        assert!(authorize_admin_request(
            &root,
            42,
            Some("current-token"),
            &mut server
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_server_action_request_replays_its_result_without_a_second_action() {
        let mut server = ServerState::default();
        let response = serde_json::json!({
            "type": "mod-operation-result",
            "requestId": "request-1",
            "success": true,
            "authorized": true,
            "message": "Server mod action queued"
        });
        remember_completed_action(
            &mut server,
            42,
            "request-1",
            "example",
            "reset",
            response.clone(),
        );

        assert_eq!(
            cached_action_response(&mut server, 42, "request-1", "example", "reset"),
            Ok(Some(response))
        );
        assert!(cached_action_response(&mut server, 42, "request-1", "other", "reset").is_err());
    }

    #[test]
    fn remote_log_mirror_keeps_only_a_bounded_complete_tail() {
        let root = temporary_directory("remote-log-mirror");
        let path = root.join("server.log");
        let line = "server log line 1234567890\n";
        let text = line.repeat((MAX_REMOTE_LOG_MIRROR_BYTES as usize / line.len()) + 4096);

        assert!(append_remote_log(&path, &text).unwrap());
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() <= RETAIN_REMOTE_LOG_MIRROR_BYTES as usize);
        assert!(bytes.len() < MAX_REMOTE_LOG_MIRROR_BYTES as usize);
        assert!(bytes.ends_with(b"\n"));
        let _ = fs::remove_dir_all(root);
    }
}
