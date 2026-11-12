use super::native_plugin_abi::{NATIVE_STATUS_UNAVAILABLE, NativePluginServiceLifetime};
use mod_loader::ModEnvironment;
use std::{cell::RefCell, collections::HashMap, ffi::c_void};
/// Immutable control-plane authority. No AppState/RefCell is retained by this
/// object and native callbacks dispatch directly inside the provider.
pub(crate) struct MapOverlayServices {
    client: bool,
    root: std::path::PathBuf,
}
impl MapOverlayServices {
    pub(crate) fn new(env: &ModEnvironment, is_server: bool) -> Self {
        Self {
            client: !is_server,
            root: env.game_dir().to_path_buf().into(),
        }
    }
}
impl NativePluginServiceLifetime for MapOverlayServices {
    fn publish_notice(
        &self,
        package: &str,
        _generation: u64,
        id: &str,
        title: &str,
        message: &str,
        level: u32,
    ) -> i32 {
        if !self.client {
            return NATIVE_STATUS_UNAVAILABLE;
        }
        match crate::notice_delivery::publish(&self.root, package, id, title, message, level) {
            Ok(()) => 0,
            Err(_) => 7,
        }
    }
    unsafe fn query_service(
        &self,
        package: &str,
        generation: u64,
        name: &str,
        version: u32,
        capacity: u32,
        output: *mut c_void,
    ) -> i32 {
        if !self.client {
            return NATIVE_STATUS_UNAVAILABLE;
        }
        unsafe {
            shroudforge_runtime_bridge::query_map_overlay_service(
                package, generation, name, version, capacity, output,
            )
        }
    }
    fn revoke(&self, package: &str, generation: u64) -> i32 {
        shroudforge_runtime_bridge::revoke_map_overlay_services(package, generation)
    }
    fn drain(&self, package: &str, generation: u64, timeout: u32) -> i32 {
        shroudforge_runtime_bridge::drain_map_overlay_services(package, generation, timeout)
    }
}
impl super::AppState {
    pub(crate) fn install_map_overlay_services(&self) -> Result<(), String> {
        self.set_native_plugin_service_lifetime(std::sync::Arc::new(MapOverlayServices::new(
            self.env(),
            self.is_server(),
        )))
    }
}

static LUA_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn release_before_remove(
    owners: &RefCell<HashMap<String, u64>>,
    id: &str,
    release: impl FnOnce(u64),
) -> Option<u64> {
    let generation = owners.borrow().get(id).copied()?;
    release(generation);
    let removed = owners.borrow_mut().remove(id);
    debug_assert_eq!(removed, Some(generation));
    Some(generation)
}

impl super::AppState {
    pub(crate) fn lua_map_owner(&self, id: &str) -> u64 {
        *self
            .map_lua_owners
            .borrow_mut()
            .entry(id.to_owned())
            .or_insert_with(|| {
                (1u64 << 63) | LUA_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            })
    }
    pub(crate) fn release_lua_map_owner(&self, id: &str) {
        release_before_remove(&self.map_lua_owners, id, |generation| {
            let revoke = shroudforge_runtime_bridge::revoke_map_overlay_services(id, generation);
            let drain = shroudforge_runtime_bridge::drain_map_overlay_services(id, generation, 250);
            if revoke != 0 || drain != 0 {
                tracing::warn!(
                    mod_id = id,
                    revoke,
                    drain,
                    "Native map owner retained pending drain"
                );
            }
        });
    }

    pub(crate) fn release_all_lua_map_owners(&self) {
        let ids = self
            .map_lua_owners
            .borrow()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for id in ids {
            self.release_lua_map_owner(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::release_before_remove;
    use std::{cell::RefCell, collections::HashMap};

    #[test]
    fn native_owner_is_released_before_lua_generation_is_retired() {
        let owners = RefCell::new(HashMap::from([("general-client-mod".to_owned(), 42_u64)]));
        let mut released = None;
        let retired = release_before_remove(&owners, "general-client-mod", |generation| {
            assert_eq!(owners.borrow().get("general-client-mod"), Some(&generation));
            released = Some(generation);
        });
        assert_eq!(released, Some(42));
        assert_eq!(retired, Some(42));
        assert!(!owners.borrow().contains_key("general-client-mod"));
    }

    #[test]
    fn missing_lua_generation_does_not_issue_a_native_release() {
        let owners = RefCell::new(HashMap::new());
        let mut released = false;
        assert_eq!(
            release_before_remove(&owners, "missing", |_| released = true),
            None
        );
        assert!(!released);
    }
}
