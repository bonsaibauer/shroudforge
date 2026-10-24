use crate::windows::provider::provider;
use crate::windows::provider::{PROVIDER_ERROR, observed_abi};
use crate::windows::resources::resource_status;

/// Copies the native menu's current cache without reading or refreshing game data.
pub fn selected_character_name() -> Option<String> {
    let fetch = provider()?.selected_character_name?;
    let mut buffer = [0u8; 1024];
    let required = unsafe { fetch(buffer.as_mut_ptr().cast(), buffer.len()) };
    if !(2..=buffer.len()).contains(&required)
        || buffer[required - 1] != 0
        || buffer[..required - 1].contains(&0)
    {
        return None;
    }
    std::str::from_utf8(&buffer[..required - 1])
        .ok()
        .map(str::to_owned)
}

/// Copies the same native GameFlow cache consumed by the Modloader status.
pub fn is_main_menu() -> Option<bool> {
    let read = provider()?.main_menu_state?;
    match unsafe { read() } {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

pub fn report() -> serde_json::Value {
    let Some(provider) = provider() else {
        return serde_json::json!({"available":false,"abi":observed_abi(),"reason":PROVIDER_ERROR.get().map(String::as_str).unwrap_or("provider-unavailable"),"mainMenu":null});
    };
    let mut buffer = [0i8; 4096];
    unsafe {
        (provider.status)(buffer.as_mut_ptr(), buffer.len());
    }
    let bytes: Vec<u8> = buffer
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect();
    let detail = String::from_utf8_lossy(&bytes).into_owned();
    // The dispatcher exposes this explicit state before its first local
    // actor-world tick. Once that tick has occurred, its status changes to
    // ready or stale-game-thread, so a later pause is not mistaken for the
    // initial menu wait.
    let awaiting_world = detail.contains("game_thread=installed-awaiting-world");
    let main_menu = is_main_menu();
    serde_json::json!({"available":true,"abi":provider.abi,"initialized":true,"ready":unsafe{(provider.ready)()},"writable":unsafe{(provider.can_write)()},"awaitingWorld":awaiting_world,"mainMenu":main_menu,"detail":detail,"resourceService":resource_status(),"selectedCharacterName":selected_character_name()})
}
