use super::IngameRuntime;

impl IngameRuntime {
    pub(super) fn restore_previous_runtime(&mut self, active_ids: &[String]) {
        for item in &mut self.lifecycle {
            if !active_ids.iter().any(|id| id == &item.id) || item.cleanup_pending {
                continue;
            }
            if item.active {
                continue;
            }
            if let Err(error) = item
                .activate_runtime_mod(&self.runner.lua, None)
                .and_then(|result| result)
            {
                tracing::error!(target:"shroudforge::runtime", mod_id=%item.id, "Could not restore runtime mod after refresh rollback: {error}");
                if let Err(cleanup) = item.rollback_failed_activation(&self.runner.lua) {
                    tracing::error!(target:"shroudforge::runtime", mod_id=%item.id,
                            "Refresh rollback cleanup is pending and will be retried: {cleanup}");
                }
                continue;
            }
            item.active = true;
        }
        self.publish_status(true);
    }
}
