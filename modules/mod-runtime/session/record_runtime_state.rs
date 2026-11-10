use super::*;
impl AppState {
    pub(crate) fn set_runtime_mod_active(&self, id: &str, active: bool) {
        let native_dll_state = self.native_dll_state(id);
        let mut active_mods = self.runtime_active_mods.borrow_mut();
        if active {
            active_mods.insert(id.to_owned());
        } else {
            active_mods.remove(id);
        }
        drop(active_mods);
        // The mod's DLLs read game state under the same condition as its Lua.
        let runtime = self.env().mod_registry().get(id).is_some_and(|r#mod| {
            r#mod.info().capabilities.contains(&mod_loader::Capability::Runtime)
        });
        crate::native_bridge::set_game_access(id, active && runtime && self.is_client());
        if !active {
            self.loaded_resource_indexes.borrow_mut().remove_owner(id);
        }
        if let Some(status) = native_dll_state {
            self.report_native_dll_effect(&status);
        } else {
            let mut effects = self.runtime_effects.borrow_mut();
            if effects
                .get(id)
                .is_some_and(|effect| effect["state"] != "restart-required")
            {
                effects.remove(id);
            }
        }
    }

    pub(crate) fn runtime_mod_is_active(&self, id: &str) -> bool {
        self.runtime_active_mods.borrow().contains(id)
    }

    pub(crate) fn runtime_ownership(
        &self,
    ) -> Rc<RefCell<crate::lua::ingame::overrides::claim_runtime_storage::OwnershipStore>> {
        self.runtime_ownership.clone()
    }

    pub(crate) fn runtime_write_fault(&self) -> Option<String> {
        let session = crate::lua::ingame::runtime_provider::world_session_id();
        self.runtime_write_fault
            .borrow()
            .as_ref()
            .filter(|(original, _)| session != 0 && *original == session)
            .map(|(_, reason)| reason.clone())
    }

    pub(crate) fn record_uncertain_runtime_write(&self, session: u64, reason: &str) {
        if session == 0 {
            return;
        }
        let mut fault = self.runtime_write_fault.borrow_mut();
        if fault
            .as_ref()
            .is_some_and(|(original, _)| *original == session)
        {
            return;
        }
        *fault = Some((session, reason.to_owned()));
    }

    pub(crate) fn report_runtime_effect(&self, id: &str, state: &str, detail: &str) {
        let detail = detail.chars().take(240).collect::<String>();
        let mut effects = self.runtime_effects.borrow_mut();
        if effects.get(id).is_some_and(|previous| {
            previous["state"].as_str() == Some(state)
                && previous["detail"].as_str() == Some(detail.as_str())
        }) {
            return;
        }
        let value = serde_json::json!({
            "state": state,
            "processTarget": if self.is_server() { "server" } else { "client" },
            "scope": "this-process",
            "detail": detail,
            "updatedAt": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });
        effects.insert(id.to_owned(), value);
    }

    pub(crate) fn runtime_effects(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.runtime_effects
                .borrow()
                .iter()
                .map(|(id, value)| (id.clone(), value.clone()))
                .collect(),
        )
    }
}
