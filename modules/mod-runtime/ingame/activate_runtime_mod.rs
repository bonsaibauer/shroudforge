use super::Lifecycle;
use crate::session::AppState;
use mlua::Function;
impl Lifecycle {
    // The outer result preserves initial startup's registry-lookup failure boundary.
    // The inner result is the mod callback outcome handled by each caller.
    pub(super) fn activate_runtime_mod(
        &mut self,
        lua: &mlua::Lua,
        diagnostics: Option<&mut shroudforge_runtime_diagnostics::Session>,
    ) -> mlua::Result<mlua::Result<()>> {
        self.active = true;
        lua.app_data_ref::<AppState>()
            .unwrap()
            .set_runtime_mod_active(&self.id, true);
        let native = || {
            lua.app_data_ref::<AppState>()
                .unwrap()
                .activate_native_plugin(&self.id)
                .map_err(mlua::Error::runtime)
        };
        let Some(key) = self.load.as_ref() else {
            return Ok(native());
        };
        let started = std::time::Instant::now();
        if diagnostics.is_some() {
            tracing::debug!(target:"shroudforge::runtime",mod_id=%self.id,"Runtime mod on_load started");
        }
        let result = lua.registry_value::<Function>(key)?.call::<()>(());
        if let Some(diagnostics) = diagnostics {
            diagnostics.measure(
                "mods",
                &self.id,
                "on_load",
                started.elapsed(),
                result.is_err(),
            );
            if result.is_ok() {
                tracing::debug!(target:"shroudforge::runtime",mod_id=%self.id,elapsed_ms=started.elapsed().as_millis(),"Runtime mod on_load completed");
            }
        }
        Ok(result.and_then(|()| native()))
    }
    pub(super) fn rollback_failed_activation(&mut self, lua: &mlua::Lua) -> mlua::Result<()> {
        self.try_unload(lua)?;
        lua.app_data_ref::<AppState>()
            .unwrap()
            .set_runtime_mod_active(&self.id, false);
        Ok(())
    }
}
