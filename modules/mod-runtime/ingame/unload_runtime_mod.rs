use super::Lifecycle;
use mlua::Function;

impl Lifecycle {
    pub(crate) fn try_unload(&mut self, lua: &mlua::Lua) -> mlua::Result<()> {
        // Keep the active scope and its checked writer permissions until cleanup
        // confirms that all temporary engine inputs have been released.
        self.cleanup_pending = true;
        let callback = self.unload.as_ref().map_or(Ok(()), |key| {
            lua.registry_value::<Function>(key)?.call::<()>(())
        });
        let cleanup =
            crate::env::runtime_overrides::cleanup(lua, &self.id).map_err(mlua::Error::runtime);
        // The game gets back what the mod reserved, sent again if it starts again.
        crate::env::shroudforge::release_input_reservations(&self.id);
        callback?;
        cleanup?;
        self.cleanup_pending = false;
        self.active = false;
        Ok(())
    }
}
