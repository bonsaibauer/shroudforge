use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RemoteServerConnectionState {
    Connecting = 0,
    Connected = 1,
    Disconnected = 2,
}

static NETWORK_REMOTE_SERVER_STATE: AtomicU8 = AtomicU8::new(0);

static NETWORK_ACKS_REPORTED: AtomicU64 = AtomicU64::new(0);
static NETWORK_SEND_FAILURES_REPORTED: AtomicU64 = AtomicU64::new(0);
static NETWORK_PROBE_TIMEOUTS_REPORTED: AtomicU64 = AtomicU64::new(0);
static NETWORK_SERVICE_STATE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static NETWORK_PROBE_OUTAGE_REPORTED: AtomicBool = AtomicBool::new(false);
static NETWORK_ACKNOWLEDGED_PEER: AtomicU64 = AtomicU64::new(0);
static NETWORK_REMOTE_SERVER_PEER: AtomicU64 = AtomicU64::new(0);
static NETWORK_PROBE_LAST_FAILURE: std::sync::Mutex<Option<std::time::Instant>> =
    std::sync::Mutex::new(None);

pub fn remote_server_connection_state() -> RemoteServerConnectionState {
    match NETWORK_REMOTE_SERVER_STATE.load(Ordering::Acquire) {
        1 => RemoteServerConnectionState::Connected,
        2 => RemoteServerConnectionState::Disconnected,
        _ => RemoteServerConnectionState::Connecting,
    }
}

fn network_probe_failure_started() -> bool {
    let mut last_failure = NETWORK_PROBE_LAST_FAILURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *last_failure = Some(std::time::Instant::now());
    !NETWORK_PROBE_OUTAGE_REPORTED.swap(true, Ordering::AcqRel)
}

fn network_probe_recovered() -> bool {
    let mut last_failure = NETWORK_PROBE_LAST_FAILURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let recovered = last_failure
        .map(|last| last.elapsed() >= std::time::Duration::from_secs(15))
        .unwrap_or(false);
    if recovered {
        *last_failure = None;
        NETWORK_PROBE_OUTAGE_REPORTED.swap(false, Ordering::AcqRel)
    } else {
        false
    }
}

pub fn report_network_health(
    authorized_clients: &[String],
    restrict_network_clients: bool,
    is_server: impl FnOnce() -> bool,
) {
    crate::service_tick(authorized_clients, restrict_network_clients);
    let network = crate::status();
    let remote_server_peer = network
        .remote_dedicated_server_steam_id
        .as_deref()
        .and_then(|peer| peer.parse::<u64>().ok())
        .unwrap_or_default();
    let previous_remote_server_peer =
        NETWORK_REMOTE_SERVER_PEER.swap(remote_server_peer, Ordering::AcqRel);
    let previous_remote_server_state = remote_server_connection_state();
    let remote_server_state = if remote_server_peer != 0 {
        RemoteServerConnectionState::Connected
    } else if previous_remote_server_peer != 0
        || previous_remote_server_state == RemoteServerConnectionState::Disconnected
        || network.service_ready
    {
        RemoteServerConnectionState::Disconnected
    } else {
        RemoteServerConnectionState::Connecting
    };
    NETWORK_REMOTE_SERVER_STATE.store(remote_server_state as u8, Ordering::Release);
    if remote_server_state != previous_remote_server_state {
        match remote_server_state {
            RemoteServerConnectionState::Connecting => tracing::info!(
                target: "shroudforge::network",
                "Steam P2P service is starting; remote server status is pending"
            ),
            RemoteServerConnectionState::Connected => tracing::info!(
                target: "shroudforge::network",
                peer = remote_server_peer,
                "Remote server P2P health probe confirmed"
            ),
            RemoteServerConnectionState::Disconnected => {
                if previous_remote_server_state == RemoteServerConnectionState::Connected {
                    tracing::warn!(
                        target: "shroudforge::network",
                        peer = previous_remote_server_peer,
                        "Remote server P2P health probe expired"
                    );
                } else {
                    tracing::warn!(
                        target: "shroudforge::network",
                        "Steam P2P service is ready but no remote server health probe is confirmed"
                    );
                }
            }
        }
    }
    let network_state = if !network.available {
        2
    } else if network.service_ready {
        1
    } else {
        3
    };
    if NETWORK_SERVICE_STATE.swap(network_state, Ordering::AcqRel) != network_state {
        if network_state == 1 {
            if is_server() {
                tracing::info!(target: "shroudforge::network", role = "server", "Network service is ready. Automatic Steam session acceptance and authenticated-peer health probes are active");
            } else {
                tracing::info!(target: "shroudforge::network", role = "client", "Network service is ready. Automatic Steam session acceptance is active and remote server discovery is listening");
            }
        } else {
            tracing::warn!(target: "shroudforge::network", available = network.available, service_ready = network.service_ready, "Network service is waiting for Steam Networking Messages");
        }
    }
    let stats = crate::service_stats();
    let previous_acks = NETWORK_ACKS_REPORTED.swap(stats.probes_acknowledged, Ordering::AcqRel);
    let previous_failures =
        NETWORK_SEND_FAILURES_REPORTED.swap(stats.probe_send_failures, Ordering::AcqRel);
    if stats.probe_send_failures > previous_failures {
        if network_probe_failure_started() {
            tracing::warn!(target: "shroudforge::network", result = stats.last_send_result.unwrap_or_default(), total_failures = stats.probe_send_failures, "Steam P2P health probe send failed. Check the peer identity and Steam session state");
        }
    }
    let previous_timeouts =
        NETWORK_PROBE_TIMEOUTS_REPORTED.swap(stats.probe_timeouts, Ordering::AcqRel);
    if stats.probe_timeouts > previous_timeouts {
        if network_probe_failure_started() {
            tracing::warn!(target: "shroudforge::network", total_timeouts = stats.probe_timeouts, timeout_seconds = 15, "Steam P2P health probe received no acknowledgement for 15 seconds");
        }
    }
    if stats.probes_acknowledged > previous_acks {
        let peer = stats.last_acknowledged_peer.as_deref().unwrap_or("unknown");
        let peer_id = peer.parse::<u64>().unwrap_or_default();
        let previous_peer = NETWORK_ACKNOWLEDGED_PEER.swap(peer_id, Ordering::AcqRel);
        let recovered = network_probe_recovered();
        if recovered || previous_acks == 0 || (peer_id != 0 && peer_id != previous_peer) {
            tracing::info!(target: "shroudforge::network", peer = peer, round_trip_ms = stats.last_round_trip_ms.unwrap_or_default(), total_acknowledged = stats.probes_acknowledged, "Steam P2P health probe confirmed");
        }
    }
}
