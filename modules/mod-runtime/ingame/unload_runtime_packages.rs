use super::IngameRuntime;
use crate::env::AppState;

impl IngameRuntime {
    pub(super) fn unload_runtime_packages(&mut self, mod_ids: &[String]) -> Result<(), String> {
        let mut unloaded_ids = Vec::new();
        for item in self.lifecycle.iter_mut().rev() {
            if !mod_ids.iter().any(|id| id == &item.id) || !item.active {
                continue;
            }
            self.diagnostics.release_mod_host(&item.id);
            if let Err(error) = item.try_unload(&self.runner.lua) {
                let mod_id = item.id.clone();
                self.restore_previous_runtime(&unloaded_ids);
                return Err(format!("mod '{mod_id}' on_unload failed: {error}"));
            }
            self.runner
                .lua
                .app_data_ref::<AppState>()
                .unwrap()
                .set_runtime_mod_active(&item.id, false);
            unloaded_ids.push(item.id.clone());
            let native_stop = self
                .runner
                .lua
                .app_data_ref::<AppState>()
                .unwrap()
                .stop_native_plugin(&item.id);
            if let Err(error) = native_stop {
                let mod_id = item.id.clone();
                self.restore_previous_runtime(&unloaded_ids);
                return Err(format!(
                    "mod '{mod_id}' native plugin teardown prevented package unload: {error}"
                ));
            }
        }
        self.publish_status(true);
        Ok(())
    }
}
