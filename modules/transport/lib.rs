//! Built-in Steam Networking Messages transport for the in-process runtime.
//!
//! The game owns Steam initialization and callback pumping. This crate only
//! resolves the initialized client or game-server interfaces exported by the
//! loaded Steam API DLL and passes bounded messages through the native runtime.

pub const MAX_MESSAGE_BYTES: usize = 512 * 1024;
/// Reserved for the Modloader remote UI. 65535 remains Network health.
pub const REMOTE_UI_CHANNEL: i32 = 65_534;
/// The largest Modloader remote UI message. One larger than a Steam message
/// (`MAX_MESSAGE_BYTES`, Steam's own limit) is sent in fragments and put back
/// together on arrival.
pub const MAX_REMOTE_UI_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// Steam's `k_EResultLimitExceeded`: the peer's send buffer is full.
const STEAM_RESULT_LIMIT_EXCEEDED: i32 = 25;

#[cfg(windows)]
static RECEIVE_BUFFER: std::sync::OnceLock<std::sync::Mutex<Vec<u8>>> = std::sync::OnceLock::new();
static REMOTE_UI_OUTBOX: std::sync::OnceLock<
    std::sync::Mutex<pace_remote_ui_sends::RemoteUiOutbox>,
> = std::sync::OnceLock::new();
static REMOTE_UI_REASSEMBLY: std::sync::OnceLock<
    std::sync::Mutex<reassemble_remote_ui_message::RemoteUiReassembly>,
> = std::sync::OnceLock::new();

#[derive(Debug, Clone)]
pub struct Status {
    pub available: bool,
    pub local_steam_id: Option<String>,
    pub local_dedicated_server_steam_id: Option<String>,
    pub remote_dedicated_server_steam_id: Option<String>,
    pub service_ready: bool,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub peer_steam_id: String,
    pub payload: Vec<u8>,
    pub reliable: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceStats {
    pub probes_sent: u64,
    pub probe_send_failures: u64,
    pub probes_acknowledged: u64,
    pub last_acknowledged_peer: Option<String>,
    pub last_round_trip_ms: Option<u64>,
    pub last_send_result: Option<i32>,
    pub probe_timeouts: u64,
}

pub fn status() -> Status {
    #[cfg(windows)]
    unsafe {
        type NetworkStatus = unsafe extern "C" fn(*mut u64) -> i32;
        let Some(address) = symbol(b"KfcRuntimeNetworkStatus\0") else {
            return unavailable();
        };
        let status: NetworkStatus = std::mem::transmute(address);
        let mut local_id = 0;
        if status(&mut local_id) != 1 {
            return unavailable();
        }
        type LocalServer = unsafe extern "C" fn(*mut u64) -> i32;
        let local_dedicated_server_steam_id =
            symbol(b"KfcRuntimeNetworkLocalServer\0").and_then(|address| {
                let discover: LocalServer = std::mem::transmute(address);
                let mut server_id = 0;
                (discover(&mut server_id) == 1 && server_id != 0).then(|| server_id.to_string())
            });
        type ServiceReady = unsafe extern "C" fn() -> i32;
        let service_ready = symbol(b"KfcRuntimeNetworkServiceReady\0").is_some_and(|address| {
            let ready: ServiceReady = std::mem::transmute(address);
            ready() == 1
        });
        type RemoteServer = unsafe extern "C" fn(*mut u64) -> i32;
        let remote_dedicated_server_steam_id =
            symbol(b"KfcRuntimeNetworkRemoteServer\0").and_then(|address| {
                let discover: RemoteServer = std::mem::transmute(address);
                let mut server_id = 0;
                (discover(&mut server_id) == 1 && server_id != 0).then(|| server_id.to_string())
            });
        return Status {
            available: true,
            local_steam_id: (local_id != 0).then(|| local_id.to_string()),
            local_dedicated_server_steam_id,
            remote_dedicated_server_steam_id,
            service_ready,
        };
    }
    #[cfg(not(windows))]
    unavailable()
}

pub fn send(
    peer_steam_id: u64,
    payload: &[u8],
    channel: i32,
    reliable: bool,
) -> Result<(), String> {
    send_on_channel(peer_steam_id, payload, channel, reliable, false)
}

/// Sends a remote UI message, in fragments when it is larger than a Steam
/// message, after those still waiting for that peer (`flush_remote_ui`).
pub(crate) fn send_remote_ui(peer_steam_id: u64, payload: &[u8]) -> Result<(), String> {
    if payload.len() > MAX_REMOTE_UI_MESSAGE_BYTES {
        return Err(format!("remote UI message exceeds {MAX_REMOTE_UI_MESSAGE_BYTES} bytes"));
    }
    let pieces = if payload.len() <= MAX_MESSAGE_BYTES {
        vec![payload.to_vec()]
    } else {
        split_remote_ui_message::split_remote_ui_message(payload, uuid::Uuid::new_v4().as_u128())
    };
    REMOTE_UI_OUTBOX
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .send(peer_steam_id, pieces, send_remote_ui_piece)
}

/// Sends the remote UI messages still waiting for their peers; the peers
/// whose send failed, with the error.
pub(crate) fn flush_remote_ui() -> Vec<(u64, String)> {
    REMOTE_UI_OUTBOX
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .flush(send_remote_ui_piece)
}

fn send_remote_ui_piece(peer_steam_id: u64, piece: &[u8]) -> pace_remote_ui_sends::PieceSent {
    use pace_remote_ui_sends::PieceSent;
    match send_result(peer_steam_id, piece, REMOTE_UI_CHANNEL, true, true) {
        Ok(1) => PieceSent::Sent,
        Ok(STEAM_RESULT_LIMIT_EXCEEDED) => PieceSent::Full,
        Ok(result) => PieceSent::Failed(format!("Steam returned EResult {result}")),
        Err(error) => PieceSent::Failed(error),
    }
}

fn send_on_channel(
    peer_steam_id: u64,
    payload: &[u8],
    channel: i32,
    reliable: bool,
    remote_ui: bool,
) -> Result<(), String> {
    match send_result(peer_steam_id, payload, channel, reliable, remote_ui)? {
        1 => Ok(()),
        result => Err(format!("Steam returned EResult {result}")),
    }
}

/// Steam's result for one message (1: sent), or an error before Steam.
fn send_result(
    peer_steam_id: u64,
    payload: &[u8],
    channel: i32,
    reliable: bool,
    remote_ui: bool,
) -> Result<i32, String> {
    if peer_steam_id == 0 {
        return Err("peer Steam ID must be nonzero".into());
    }
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(format!("payload exceeds {MAX_MESSAGE_BYTES} bytes"));
    }
    validate_channel(channel, remote_ui)?;
    #[cfg(windows)]
    unsafe {
        type NetworkSend = unsafe extern "C" fn(u64, *const u8, usize, i32, i32) -> i32;
        let address = symbol(b"KfcRuntimeNetworkSend\0")
            .ok_or_else(|| "Steam Networking runtime module is unavailable".to_owned())?;
        let send: NetworkSend = std::mem::transmute(address);
        let result = send(
            peer_steam_id,
            payload.as_ptr(),
            payload.len(),
            channel,
            if reliable { 1 } else { 0 },
        );
        if result < 0 {
            Err("Steam Networking Messages interface is unavailable".into())
        } else {
            Ok(result)
        }
    }
    #[cfg(not(windows))]
    Err("Steam Networking Messages is supported only by the Windows game runtime".into())
}

/// Advances the built-in Network handshake and P2P health probe. Steam callback
/// delivery remains owned by the game; this only consumes callbacks already run.
pub fn service_tick(authorized_clients: &[String], restrict_clients: bool) {
    #[cfg(windows)]
    unsafe {
        type NetworkTick = unsafe extern "C" fn(*const u64, usize, i32);
        if let Some(address) = symbol(b"KfcRuntimeNetworkServiceTick\0") {
            let tick: NetworkTick = std::mem::transmute(address);
            let ids = authorized_clients
                .iter()
                .filter_map(|peer| peer.parse::<u64>().ok())
                .collect::<Vec<_>>();
            tick(
                ids.as_ptr(),
                ids.len(),
                if restrict_clients { 1 } else { 0 },
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (authorized_clients, restrict_clients);
}

pub fn service_shutdown() {
    #[cfg(windows)]
    unsafe {
        type NetworkShutdown = unsafe extern "C" fn();
        if let Some(address) = symbol(b"KfcRuntimeNetworkServiceShutdown\0") {
            let shutdown: NetworkShutdown = std::mem::transmute(address);
            shutdown();
        }
    }
}

pub fn service_stats() -> ServiceStats {
    #[cfg(windows)]
    unsafe {
        type NetworkStats = unsafe extern "C" fn(
            *mut u64,
            *mut u64,
            *mut u64,
            *mut u64,
            *mut u64,
            *mut i32,
            *mut u64,
        );
        if let Some(address) = symbol(b"KfcRuntimeNetworkServiceStats\0") {
            let read: NetworkStats = std::mem::transmute(address);
            let mut probes_sent = 0;
            let mut probe_send_failures = 0;
            let mut probes_acknowledged = 0;
            let mut peer = 0;
            let mut round_trip_ms = 0;
            let mut last_send_result = 0;
            let mut probe_timeouts = 0;
            read(
                &mut probes_sent,
                &mut probe_send_failures,
                &mut probes_acknowledged,
                &mut peer,
                &mut round_trip_ms,
                &mut last_send_result,
                &mut probe_timeouts,
            );
            return ServiceStats {
                probes_sent,
                probe_send_failures,
                probes_acknowledged,
                last_acknowledged_peer: (peer != 0).then(|| peer.to_string()),
                last_round_trip_ms: (peer != 0).then_some(round_trip_ms),
                last_send_result: (probes_sent > 0 || probe_send_failures > 0)
                    .then_some(last_send_result),
                probe_timeouts,
            };
        }
    }
    ServiceStats::default()
}

pub fn accept(peer_steam_id: u64) -> Result<bool, String> {
    if peer_steam_id == 0 {
        return Err("peer Steam ID must be nonzero".into());
    }
    #[cfg(windows)]
    unsafe {
        type NetworkAccept = unsafe extern "C" fn(u64) -> i32;
        let address = symbol(b"KfcRuntimeNetworkAccept\0")
            .ok_or_else(|| "Steam Networking runtime module is unavailable".to_owned())?;
        let accept: NetworkAccept = std::mem::transmute(address);
        match accept(peer_steam_id) {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err("Steam Networking Messages interface is unavailable".into()),
        }
    }
    #[cfg(not(windows))]
    Err("Steam Networking Messages is supported only by the Windows game runtime".into())
}

/// Checks the native server's current authenticated-player and Network allowlist snapshot.
pub fn is_authorized_server_peer(peer_steam_id: u64) -> bool {
    #[cfg(windows)]
    unsafe {
        type PeerAuthorized = unsafe extern "C" fn(u64) -> i32;
        return symbol(b"KfcRuntimeNetworkPeerAuthorized\0").is_some_and(|address| {
            let check: PeerAuthorized = std::mem::transmute(address);
            check(peer_steam_id) == 1
        });
    }
    #[cfg(not(windows))]
    false
}

/// Steam IDs authenticated by Enshrouded and still connected to this Dedicated Server.
pub fn connected_peers() -> Result<Vec<String>, String> {
    #[cfg(windows)]
    unsafe {
        type NetworkPeers = unsafe extern "C" fn(*mut u64, usize, *mut usize) -> i32;
        let address = symbol(b"KfcRuntimeNetworkConnectedPeers\0")
            .ok_or_else(|| "authenticated server peer discovery is unavailable".to_owned())?;
        let read: NetworkPeers = std::mem::transmute(address);
        let mut peers = [0u64; 64];
        let mut count = 0usize;
        if read(peers.as_mut_ptr(), peers.len(), &mut count) != 1 || count > peers.len() {
            return Err("could not read authenticated Enshrouded server peers".into());
        }
        return Ok(peers[..count].iter().map(u64::to_string).collect());
    }
    #[cfg(not(windows))]
    Err("authenticated server peer discovery is supported only on Windows".into())
}

pub fn receive(channel: i32) -> Result<Option<Message>, String> {
    receive_on_channel(channel, false)
}

/// The next whole remote UI message: fragments are kept until their message
/// is complete.
pub(crate) fn receive_remote_ui() -> Result<Option<Message>, String> {
    use reassemble_remote_ui_message::Reassembled;
    loop {
        let Some(message) = receive_on_channel(REMOTE_UI_CHANNEL, true)? else {
            return Ok(None);
        };
        let Ok(peer) = message.peer_steam_id.parse::<u64>() else {
            return Ok(Some(message));
        };
        let reassembled = REMOTE_UI_REASSEMBLY
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .accept(peer, message.payload, std::time::Instant::now());
        match reassembled {
            Reassembled::Message(payload) => {
                return Ok(Some(Message {
                    peer_steam_id: message.peer_steam_id,
                    payload,
                    reliable: message.reliable,
                }));
            }
            Reassembled::Pending => {}
            Reassembled::Invalid(reason) => {
                tracing::warn!(
                    target: "shroudforge::network",
                    peer,
                    reason,
                    "Remote UI fragment dropped"
                );
            }
        }
    }
}

fn receive_on_channel(channel: i32, remote_ui: bool) -> Result<Option<Message>, String> {
    validate_channel(channel, remote_ui)?;
    #[cfg(windows)]
    unsafe {
        type NetworkReceive =
            unsafe extern "C" fn(i32, *mut u8, usize, *mut u64, *mut usize, *mut u32) -> i32;
        let address = symbol(b"KfcRuntimeNetworkReceive\0")
            .ok_or_else(|| "Steam Networking runtime module is unavailable".to_owned())?;
        let receive: NetworkReceive = std::mem::transmute(address);
        // Idle polling is common for Lua mods. Keep the 512 KiB native receive
        // buffer across calls and allocate an owned payload only when a packet
        // actually arrives.
        let shared =
            RECEIVE_BUFFER.get_or_init(|| std::sync::Mutex::new(vec![0u8; MAX_MESSAGE_BYTES]));
        let mut payload = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut peer_steam_id = 0;
        let mut actual = 0;
        let mut reliable = 0;
        match receive(
            channel,
            payload.as_mut_ptr(),
            payload.len(),
            &mut peer_steam_id,
            &mut actual,
            &mut reliable,
        ) {
            0 => Ok(None),
            1 if peer_steam_id != 0 && actual <= payload.len() => Ok(Some(Message {
                peer_steam_id: peer_steam_id.to_string(),
                payload: payload[..actual].to_vec(),
                reliable: reliable != 0,
            })),
            _ => Err("Steam Networking receive failed or returned an invalid message".into()),
        }
    }
    #[cfg(not(windows))]
    Err("Steam Networking Messages is supported only by the Windows game runtime".into())
}

fn validate_channel(channel: i32, remote_ui: bool) -> Result<(), String> {
    if remote_ui {
        if channel != REMOTE_UI_CHANNEL {
            return Err("invalid internal Modloader UI channel".into());
        }
    } else if !(0..=65_533).contains(&channel) {
        return Err("channel must be between 0 and 65533; 65534 is reserved for Modloader UI and 65535 for Network health checks".into());
    }
    Ok(())
}

fn unavailable() -> Status {
    Status {
        available: false,
        local_steam_id: None,
        local_dedicated_server_steam_id: None,
        remote_dedicated_server_steam_id: None,
        service_ready: false,
    }
}

#[cfg(windows)]
fn symbol(name: &[u8]) -> Option<*const ()> {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    unsafe {
        let module = GetModuleHandleW(
            "kfc-runtime.dll\0"
                .encode_utf16()
                .collect::<Vec<_>>()
                .as_ptr(),
        );
        if module.is_null() {
            return None;
        }
        GetProcAddress(module, name.as_ptr().cast()).map(|value| value as *const ())
    }
}

#[path = "lua/network.rs"]
pub mod lua;

mod remote_ui;
pub use remote_ui::tick as service_remote_ui;
mod pace_remote_ui_sends;
mod reassemble_remote_ui_message;
mod split_remote_ui_message;
mod local_bridge;
pub use local_bridge::submit as request_remote_ui;

mod read_network_allowlist;
mod read_session_peers;
mod report_network_health;
pub use read_network_allowlist::read_network_allowlist;
pub use read_session_peers::{SessionPeers, SessionPlayer, SessionRole, read_session_peers};
pub use report_network_health::{
    RemoteServerConnectionState, remote_server_connection_state, report_network_health,
};
