use super::IngameRuntime;
use crate::env::AppState;

impl IngameRuntime {
    pub fn update(&mut self, delta_seconds: f64) {
        let mut diagnostic_host = super::run_diagnostic_job::RuntimeDiagnosticHost::new(
            &self.runner,
            self.active_mod_ids(),
        );
        self.diagnostics.tick_with_host(&mut diagnostic_host);
        self.runner
            .lua
            .app_data_ref::<AppState>()
            .unwrap()
            .poll_native_dll_loads();
        self.process_runtime_reload_request();
        if std::time::Instant::now() >= self.next_status {
            let config = shroudforge_loader_config::config::read_loader_in_background(&self.root).ok();
            if config
                .as_ref()
                .and_then(|value| value.pointer("/modules/network/enabled"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                let (authorized_clients, restrict_network_clients) = config
                    .as_ref()
                    .map(shroudforge_steam_networking::read_network_allowlist)
                    .unwrap_or_else(|| (Vec::new(), true));
                shroudforge_steam_networking::report_network_health(
                    &authorized_clients,
                    restrict_network_clients,
                    || {
                        self.runner
                            .lua
                            .app_data_ref::<AppState>()
                            .unwrap()
                            .is_server()
                    },
                );
            } else {
                shroudforge_steam_networking::service_shutdown();
            }
            self.refresh_configuration();
            self.publish_status(true);
            self.next_status = std::time::Instant::now() + std::time::Duration::from_secs(1);
        }
        crate::env::shroudforge::dispatch_ui_actions(&self.runner.lua, &self.active_mod_ids());
        self.update_mod_callbacks(delta_seconds);
        let state = self.runner.lua.app_data_ref::<AppState>().unwrap();
        state.advance_loaded_resource_indexes();
    }
}
