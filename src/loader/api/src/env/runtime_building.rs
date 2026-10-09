//! Ordinary client building input. The game's existing connection carries these
//! actions; dispatch success is deliberately distinct from a replicated effect.
use super::{AppState, loader};
use mlua::{Lua, Table};
use mod_loader::Mod;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

static NEXT_OWNER: AtomicU32 = AtomicU32::new(0);
static ACTIVE_OWNER: AtomicU32 = AtomicU32::new(0);

struct RequestOwner {
    request: AtomicU32,
    id: u32,
}
impl RequestOwner {
    fn new() -> Self {
        let mut id = NEXT_OWNER.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        if id == 0 {
            id = NEXT_OWNER.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        }
        Self {
            request: AtomicU32::new(0),
            id,
        }
    }
    fn acquire(&self) -> bool {
        match ACTIVE_OWNER.compare_exchange(0, self.id, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(id) => id == self.id,
        }
    }
    fn release(&self) {
        let _ = ACTIVE_OWNER.compare_exchange(self.id, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}
impl Drop for RequestOwner {
    fn drop(&mut self) {
        cancel(self.request.load(Ordering::Relaxed));
        self.release();
    }
}

fn cancel(id: u32) -> bool {
    if id == 0 {
        return false;
    }
    #[cfg(windows)]
    if let Some(address) = symbol(b"KfcRuntimeWorldBuildingInputCancel\0") {
        let function: unsafe extern "C" fn(u32) -> bool = unsafe { std::mem::transmute(address) };
        return unsafe { function(id) };
    }
    false
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
        GetProcAddress(module, name.as_ptr()).map(|value| value as *const ())
    }
}

fn submit(
    player: u32,
    kind: u32,
    item: u32,
    material: u32,
    slot: u32,
    target_entity_id: u32,
    position: [f64; 3],
    rotation: [f64; 4],
    scale: [f64; 3],
) -> u32 {
    #[cfg(windows)]
    if let Some(address) = symbol(b"KfcRuntimeWorldBuildingInput\0") {
        let function: unsafe extern "C" fn(
            u32,
            u32,
            u32,
            u32,
            u32,
            u32,
            *const f64,
            *const f64,
            *const f64,
        ) -> u32 = unsafe { std::mem::transmute(address) };
        return unsafe {
            function(
                player,
                kind,
                item,
                material,
                slot,
                target_entity_id,
                position.as_ptr(),
                rotation.as_ptr(),
                scale.as_ptr(),
            )
        };
    }
    let _ = (
        player, kind, item, material, slot, target_entity_id, position, rotation, scale,
    );
    0
}

fn status(id: u32) -> u32 {
    #[cfg(windows)]
    if let Some(address) = symbol(b"KfcRuntimeWorldBuildingInputStatus\0") {
        let function: unsafe extern "C" fn(u32) -> u32 = unsafe { std::mem::transmute(address) };
        return unsafe { function(id) };
    }
    let _ = id;
    0
}

fn vector<const N: usize>(table: &Table, name: &str, default: [f64; N]) -> mlua::Result<[f64; N]> {
    let Some(values) = table.raw_get::<Option<Table>>(name)? else {
        return Ok(default);
    };
    let mut out = default;
    for (i, value) in out.iter_mut().enumerate() {
        *value = values.raw_get(i + 1)?;
        if !value.is_finite() {
            return Err(mlua::Error::runtime("building transform must be finite"));
        }
    }
    Ok(out)
}

pub(crate) fn attach(lua: &Lua, world: &Table, owner: &Mod) -> mlua::Result<()> {
    let api = lua.create_table()?;
    let owner = owner.clone();
    let request_owner = Arc::new(RequestOwner::new());
    let submit_owner = request_owner.clone();
    api.set(
        "submit",
        lua.create_function(move |lua, input: Table| {
            let state = lua.app_data_ref::<AppState>().unwrap();
            if let Some(reason) =
                loader::runtime_denial_reason(&state, &owner, "runtime.world.building.input")
            {
                return Ok((None, Some(reason)));
            }
            if state.is_server() {
                return Ok((
                    None,
                    Some("building input belongs to a connected client".into()),
                ));
            }
            let action = input.raw_get::<String>("action")?;
            let kind = match action.as_str() {
                "select" => 0,
                "place" => 1,
                "remove" => 2,
                "undo" => 3,
                "dismantle" => 4,
                _ => {
                    return Err(mlua::Error::runtime(
                        "building action must be select, place, remove, dismantle or undo",
                    ));
                }
            };
            if (kind == 1 || kind == 2 || kind == 4) && !input.contains_key("position")? {
                return Err(mlua::Error::runtime("building position is required"));
            }
            let player = input.raw_get::<u32>("player")?;
            let item = input.raw_get::<Option<u32>>("itemId")?.unwrap_or(0);
            let material = input.raw_get::<Option<u32>>("materialItemId")?.unwrap_or(0);
            let slot = input.raw_get::<Option<u32>>("slot")?.unwrap_or(0);
            let target_entity_id = input.raw_get::<Option<u32>>("targetEntityId")?.unwrap_or(0);
            if (kind == 4) != (target_entity_id != 0) {
                return Ok((None, Some("dismantle requires an exact targetEntityId".into())));
            }
            let position = vector(&input, "position", [0.; 3])?;
            let rotation = vector(&input, "rotation", [0., 0., 0., 1.])?;
            let scale = vector(&input, "scale", [1.; 3])?;
            if !submit_owner.acquire() {
                return Ok((
                    None,
                    Some("another mod owns the building input sequence".into()),
                ));
            }
            let id = submit(
                player, kind, item, material, slot, target_entity_id, position, rotation, scale,
            );
            if id == 0 {
                if submit_owner.request.load(Ordering::Relaxed) == 0 {
                    submit_owner.release();
                }
                Ok((
                    None,
                    Some("building input unavailable, busy, or request invalid".into()),
                ))
            } else {
                submit_owner.request.store(id, Ordering::Relaxed);
                Ok((Some(id), None))
            }
        })?,
    )?;
    let status_owner = request_owner.clone();
    api.set(
        "status",
        lua.create_function(move |_, id: u32| {
            if id == 0 || id != status_owner.request.load(Ordering::Relaxed) {
                return Ok("unknown");
            }
            Ok(match status(id) {
                1 => "queued",
                2 => "pressed",
                3 => "dispatched",
                4 => "timeout",
                5 => "cancelled",
                _ => "unknown",
            })
        })?,
    )?;
    api.set(
        "cancel",
        lua.create_function(move |_, id: u32| {
            if id == 0 || id != request_owner.request.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let cancelled = cancel(id);
            request_owner.request.store(0, Ordering::Relaxed);
            request_owner.release();
            Ok(cancelled)
        })?,
    )?;
    world.set("building", api)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn building_sequence_has_one_owner_until_cancel_or_drop() {
        let first = RequestOwner::new();
        let second = RequestOwner::new();
        assert!(first.acquire());
        assert!(first.acquire());
        assert!(!second.acquire());
        second.release();
        assert!(!second.acquire());
        first.release();
        assert!(second.acquire());
        drop(second);
        assert!(first.acquire());
        drop(first);
        assert_eq!(ACTIVE_OWNER.load(Ordering::Acquire), 0);
    }
}
