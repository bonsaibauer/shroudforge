use std::{
    collections::HashSet,
    sync::{Mutex, MutexGuard},
};

/// Mods whose DLLs may read game state through the host: the same condition
/// as the Lua runtime API (an active ingame runtime mod with the `runtime`
/// capability, in a game client). Kept here because DLL threads cannot read
/// the session state, which belongs to the Lua thread.
static GAME_ACCESS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn granted() -> MutexGuard<'static, Option<HashSet<String>>> {
    GAME_ACCESS.lock().unwrap_or_else(|error| error.into_inner())
}

/// Called wherever the runtime activates or deactivates a mod.
pub(crate) fn set_game_access(mod_id: &str, allowed: bool) {
    let mut granted = granted();
    let mods = granted.get_or_insert_with(HashSet::new);
    if allowed {
        mods.insert(mod_id.to_owned());
    } else {
        mods.remove(mod_id);
    }
}

pub(crate) fn has_game_access(mod_id: &str) -> bool {
    granted()
        .as_ref()
        .is_some_and(|mods| mods.contains(mod_id))
}
