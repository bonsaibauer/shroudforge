//! File mirror stream used by the local Modloader UI to view a connected server.
//!
//! The game remains the Steam owner. This small adapter transfers the existing
//! server log files and module settings to files beside the local client data.

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
const RECEIVE_BUDGET: usize = 16;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const SERVER_PEER_TIMEOUT: Duration = Duration::from_secs(30);
const LOGIN_FAILURE_LIMIT: u8 = 5;
const LOGIN_COOLDOWN: Duration = Duration::from_secs(60);
const PENDING_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PENDING_REQUESTS: usize = 32;
const RECEIVE_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(30);
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
    last_receive_error_log: Option<Instant>,
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
    admin_sessions: HashMap<u64, AdminSession>,
    login_failures: HashMap<u64, LoginFailures>,
    game: FileCursor,
    loader: FileCursor,
    config_snapshot: Option<serde_json::Value>,
    completed_actions: HashMap<(u64, String), CompletedAction>,
    completed_action_order: VecDeque<(u64, String)>,
    last_receive_error_log: Option<Instant>,
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
            state.server.game.offset = None;
            state.server.loader.offset = None;
            state.server.config_snapshot = None;
            state.server.last_receive_error_log = None;
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
            state.server.game.offset = None;
            state.server.loader.offset = None;
            state.server.config_snapshot = None;
            state.server.last_receive_error_log = None;
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
            Some("log") => receive_log(root, &value, client),
            Some("config") => {
                receive_config(root, &value["value"], client);
            }
            Some("admin-auth-result") => receive_auth_result(&value, client),
            Some("config-update-result") => receive_config_result(root, &value, client),
            Some("mods-list-result" | "mod-operation-result") => receive_mod_result(&value, client),
            _ => {}
        }
    }
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
            Some("mods-list") | Some("mod-operation") => {
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
        match value["type"].as_str() {
            Some("subscribe") => {
                let first_subscription = !server.peers.contains_key(&peer);
                server.peers.insert(peer, Instant::now());
                if first_subscription {
                    if send_initial_snapshot(root, config, peer, server) {
                        server.config_snapshot = Some(config.clone());
                        tracing::debug!(
                            target: "shroudforge::network",
                            peer,
                            "Sent initial remote Modloader snapshot"
                        );
                    } else {
                        remove_failed_peers(server, [peer]);
                    }
                }
            }
            Some("unsubscribe") => {
                server.peers.remove(&peer);
                forget_completed_actions(server, peer);
                tracing::debug!(target: "shroudforge::network", peer, "Remote UI peer unsubscribed");
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
                    if let Err(error) = send_auth_result(
                        peer,
                        request_id,
                        Some(&token),
                        Some(&group_name),
                        "",
                    ) {
                        server.admin_sessions.remove(&peer);
                        tracing::warn!(
                            target: "shroudforge::network",
                            request_id,
                            %error,
                            "Could not return server admin session token"
                        );
                    } else {
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
    server.login_failures.retain(|peer, failures| {
        crate::is_authorized_server_peer(*peer)
            && failures
                .last_attempt
                .is_some_and(|last| last.elapsed() < LOGIN_COOLDOWN)
    });
    if server.peers.is_empty() {
        server.game.offset = None;
        server.loader.offset = None;
        server.config_snapshot = None;
        return;
    }

    if !config_updated {
        send_module_changes(config, server);
    }
    if server.peers.is_empty() {
        return;
    }
    let game = server_game_log(root);
    let loader = shroudforge_loader_config::paths::current_log(root);
    let failed = broadcast_file_delta(&server.peers, &game, "game", &mut server.game);
    remove_failed_peers(server, failed);
    if server.peers.is_empty() {
        return;
    }
    let failed = broadcast_file_delta(&server.peers, &loader, "loader", &mut server.loader);
    remove_failed_peers(server, failed);
}

fn should_log_receive_error(last_logged: &mut Option<Instant>) -> bool {
    if last_logged.is_none_or(|last| last.elapsed() >= RECEIVE_ERROR_LOG_INTERVAL) {
        *last_logged = Some(Instant::now());
        true
    } else {
        false
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
        .and_then(|mods| server_view_snapshot(root, mods))
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

/// Compare and write the modules under the installation's existing config lock.
/// A stale client therefore cannot overwrite a setting saved by another client.
fn compare_and_swap_modules(
    root: &Path,
    expected: &serde_json::Value,
    modules: &serde_json::Value,
) -> (Result<(), String>, bool) {
    let mut conflict = false;
    let result = shroudforge_loader_config::config::update_loader(root, |current| {
        if current.get("modules") != Some(expected) {
            conflict = true;
            return Err("Server settings changed in another session".into());
        }
        current["modules"] = modules.clone();
        Ok(())
    });
    (result, conflict)
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
) -> Result<(serde_json::Value, serde_json::Value), String> {
    let entries = mods.as_array().ok_or("server mod snapshot must be an array")?;
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
        .map(|(id, name, version, ecosystem, changelog)| shroudforge_news::ModNotification {
            id,
            name,
            version,
            ecosystem,
            changelog,
        })
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
    let compatibility = mod_loader::compatibility_status::configuration(
        root,
        true,
        env!("CARGO_PKG_VERSION"),
        shroudforge_loader_config::runtime_status::runtime_pid(root),
    );
    let server_view = serde_json::json!({
        "notices": notices,
        "newsTemplates": news_templates,
        "gameVersion": game_version,
        "configuration": compatibility,
        "dllCandidates": mod_loader::dll_manager::discover_dll_candidates(root),
        "dllChainEntries": mod_loader::dll_manager::chain_load_entries(root),
        "hotkeyConflicts": [],
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
    root: &Path,
    config: &serde_json::Value,
    peer: u64,
    server: &mut ServerState,
) -> bool {
    let modules_sent = send_config(peer, config).is_ok();
    let (game_tail, game_offset) = read_tail(&server_game_log(root));
    let game_sent = send_log(peer, "game", "replace", game_tail).is_ok();
    if game_sent && server.game.offset.is_none() {
        server.game.offset = game_offset;
    }
    let (loader_tail, loader_offset) =
        read_tail(&shroudforge_loader_config::paths::current_log(root));
    let loader_sent = send_log(peer, "loader", "replace", loader_tail).is_ok();
    if loader_sent && server.loader.offset.is_none() {
        server.loader.offset = loader_offset;
    }
    modules_sent && game_sent && loader_sent
}

fn send_module_changes(config: &serde_json::Value, server: &mut ServerState) {
    if server.config_snapshot.as_ref() == Some(config) {
        return;
    }
    let payload = encode_json(&serde_json::json!({ "type": "config", "value": config }));
    let Ok(payload) = payload else {
        return;
    };
    let failed = send_payload_to_peers(&server.peers, &payload);
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

fn send_config(peer: u64, config: &serde_json::Value) -> Result<(), String> {
    send_json(
        peer,
        &serde_json::json!({ "type": "config", "value": config }),
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
