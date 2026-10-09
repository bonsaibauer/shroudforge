//! Built-in Steam Networking Messages transport for the in-process runtime.
//!
//! The game owns Steam initialization and callback pumping. This crate only
//! resolves the initialized client or game-server interfaces exported by the
//! loaded Steam API DLL and passes bounded messages through the native runtime.

pub const MAX_MESSAGE_BYTES: usize = 512 * 1024;

#[cfg(windows)]
static RECEIVE_BUFFER: std::sync::OnceLock<std::sync::Mutex<Vec<u8>>> = std::sync::OnceLock::new();

#[derive(Debug, Clone)]
pub struct Status {
    pub available: bool,
    pub local_steam_id: Option<String>,
    pub local_dedicated_server_steam_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub peer_steam_id: String,
    pub payload: Vec<u8>,
    pub reliable: bool,
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
        return Status {
            available: true,
            local_steam_id: (local_id != 0).then(|| local_id.to_string()),
            local_dedicated_server_steam_id,
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
    if peer_steam_id == 0 {
        return Err("peer Steam ID must be nonzero".into());
    }
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(format!("payload exceeds {MAX_MESSAGE_BYTES} bytes"));
    }
    validate_channel(channel)?;
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
        if result == 1 {
            Ok(())
        } else if result < 0 {
            Err("Steam Networking Messages interface is unavailable".into())
        } else {
            Err(format!("Steam returned EResult {result}"))
        }
    }
    #[cfg(not(windows))]
    Err("Steam Networking Messages is supported only by the Windows game runtime".into())
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
    validate_channel(channel)?;
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

fn validate_channel(channel: i32) -> Result<(), String> {
    if !(0..=65_535).contains(&channel) {
        return Err("channel must be between 0 and 65535".into());
    }
    Ok(())
}

fn unavailable() -> Status {
    Status {
        available: false,
        local_steam_id: None,
        local_dedicated_server_steam_id: None,
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
