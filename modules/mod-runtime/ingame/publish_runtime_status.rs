use super::IngameRuntime;
use crate::{API_VERSION, env::AppState};

impl IngameRuntime {
    pub(super) fn publish_status(&self, running: bool) {
        let state = self.runner.lua.app_data_ref::<AppState>().unwrap();
        let mut provider = crate::env::runtime_provider_report();
        provider["resourceIndexes"] = state.loaded_resource_indexes().diagnostics();
        provider["apiMetrics"] = shroudforge_runtime_diagnostics::operation_timings::snapshot();
        let value = serde_json::json!({
            "schemaVersion":1,"pid":std::process::id(),"apiVersion":API_VERSION,
            "updatedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            "running":running,"active":if running {self.active_mod_ids()}else{Vec::new()},
            "loaded":self.loaded,"errors":self.errors,
            "effects":state.runtime_effects(),
            "dllPlan":crate::native_dlls::applied_plan(),
            "runtimeProvider":provider
        });
        // Off the mods' loop while running; the final status at once.
        if running {
            shroudforge_loader_config::config::write_document_in_background(
                &self.root,
                shroudforge_runtime_diagnostics::documents::MOD_STATUS,
                value,
                |error| tracing::error!(target:"shroudforge::runtime","status publication failed: {error}"),
            );
        } else if let Err(error) = shroudforge_loader_config::config::write_document_now(
            &self.root,
            shroudforge_runtime_diagnostics::documents::MOD_STATUS,
            &value,
        ) {
            tracing::error!(target:"shroudforge::runtime","status publication failed: {error}");
        }
    }
}
