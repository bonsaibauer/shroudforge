use super::native_plugin_abi::{NATIVE_STATUS_UNAVAILABLE, NativePluginServiceLifetime};
use mod_loader::ModEnvironment;
use std::ffi::c_void;
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
        let services = super::selected_native_plugin_services(
            self.env().game_dir().as_std_path(),
            self.is_server(),
        )?
        .unwrap_or_else(|| super::new_native_plugin_service_lifetime(self.env(), self.is_server()));
        self.set_native_plugin_service_lifetime(services)
    }
}
