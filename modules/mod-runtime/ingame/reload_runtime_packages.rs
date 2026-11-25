use super::IngameRuntime;
use crate::env::AppState;
use mod_loader::ModEnvironment;

impl IngameRuntime {
    pub(super) fn reload_runtime_packages(&mut self, mod_ids: &[String]) -> Result<(), String> {
        self.diagnostics.release_runtime_host();
        let previously_active = self
            .lifecycle
            .iter()
            .filter(|item| item.active && !item.cleanup_pending)
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        for item in self.lifecycle.iter_mut().rev() {
            if !item.active {
                continue;
            }
            if let Err(error) = item.try_unload(&self.runner.lua) {
                let mod_id = item.id.clone();
                self.restore_previous_runtime(&previously_active);
                return Err(format!(
                    "mod '{mod_id}' cleanup prevented package refresh: {error}"
                ));
            }
            self.runner
                .lua
                .app_data_ref::<AppState>()
                .unwrap()
                .set_runtime_mod_active(&item.id, false);
        }

        let game_path = self
            .root
            .to_str()
            .ok_or_else(|| "game path is not UTF-8".to_owned())?;
        let environment = match ModEnvironment::load(game_path) {
            Ok(environment) => environment,
            Err(error) => {
                self.restore_previous_runtime(&previously_active);
                return Err(format!("updated mod package discovery failed: {error:?}"));
            }
        };
        let mut replacement =
            match Self::start_with_schema(&environment, self.api.clone(), self.file_name.clone()) {
                Ok(runtime) => runtime,
                Err(error) => {
                    self.restore_previous_runtime(&previously_active);
                    return Err(format!("updated runtime initialization failed: {error}"));
                }
            };
        for id in mod_ids {
            let enabled = environment
                .mod_registry()
                .get(id)
                .is_some_and(|item| item.info().enabled);
            let has_runtime = environment.mod_registry().get(id).is_some_and(|item| {
                item.info()
                    .capabilities
                    .iter()
                    .any(|capability| capability.requires_runtime())
            });
            if enabled
                && has_runtime
                && !replacement
                    .active_mod_ids()
                    .iter()
                    .any(|active| active == id)
            {
                let reason = replacement
                    .errors
                    .get(id)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("updated mod did not become active")
                    .to_owned();
                drop(replacement);
                self.restore_previous_runtime(&previously_active);
                return Err(format!(
                    "updated runtime mod '{id}' failed to activate: {reason}"
                ));
            }
        }

        std::mem::swap(&mut self.diagnostics, &mut replacement.diagnostics);
        std::mem::swap(&mut self.root, &mut replacement.root);
        std::mem::swap(&mut self.api, &mut replacement.api);
        std::mem::swap(&mut self.file_name, &mut replacement.file_name);
        std::mem::swap(&mut self.loaded, &mut replacement.loaded);
        std::mem::swap(&mut self.errors, &mut replacement.errors);
        std::mem::swap(&mut self.next_status, &mut replacement.next_status);
        std::mem::swap(&mut self.configurations, &mut replacement.configurations);
        std::mem::swap(&mut self.observed, &mut replacement.observed);
        std::mem::swap(&mut self.runner, &mut replacement.runner);
        std::mem::swap(&mut self.lifecycle, &mut replacement.lifecycle);
        replacement.publish_on_drop = false;
        Ok(())
    }
}
