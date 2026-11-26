use super::*;
impl AppState {
    pub(crate) fn set_runtime_mod_active(&self, id: &str, active: bool) {
        let native_dll_status = self.native_dll_status(id);
        let native_dll_state = self.native_dll_state(id);
        let generation = self.native_package_generation(id);
        let mut active_mods = self.runtime_active_mods.borrow_mut();
        if active {
            active_mods
                .entry(id.to_owned())
                .or_insert_with(|| RuntimeActivation {
                    generation: generation
                        .unwrap_or_else(super::native_plugins::next_runtime_package_generation),
                    map_overlay_used: false,
                });
        } else {
            active_mods.remove(id);
        }
        drop(active_mods);
        // The mod's DLLs read game state under the same condition as its Lua.
        let runtime = self.env().mod_registry().get(id).is_some_and(|r#mod| {
            r#mod
                .info()
                .capabilities
                .contains(&mod_loader::Capability::Runtime)
        });
        crate::native_bridge::set_game_access(id, active && runtime && self.is_client());
        if !active {
            self.loaded_resource_indexes.borrow_mut().remove_owner(id);
        }
        if let Some(status) = native_dll_status {
            self.report_native_dll_effect(&status);
        } else if let Some(state) = native_dll_state {
            let detail = match state {
                "loading" => "native DLL is still loading",
                "prepared" => "native callbacks are prepared and await package activation",
                "retained" => {
                    "native plugin remains resident because safe unload was not confirmed"
                }
                _ => "native DLL remains loaded for this game session",
            };
            self.report_runtime_effect(id, state, detail);
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
        self.runtime_active_mods.borrow().contains_key(id)
    }
    pub(crate) fn runtime_package_generation(&self, id: &str) -> Option<u64> {
        self.runtime_active_mods
            .borrow()
            .get(id)
            .map(|value| value.generation)
    }
    pub(crate) fn mark_runtime_services_used(&self, id: &str) {
        if let Some(value) = self.runtime_active_mods.borrow_mut().get_mut(id) {
            value.map_overlay_used = true;
        }
    }
    pub(crate) fn retire_runtime_services(&self, id: &str) -> Result<(), String> {
        let Some(RuntimeActivation {
            generation,
            map_overlay_used: true,
        }) = self.runtime_active_mods.borrow().get(id).copied()
        else {
            return Ok(());
        };
        let services = self.native_plugin_services.borrow().clone();
        let revoke = services.revoke(id, generation);
        if revoke != 0 {
            return Err(format!("runtime service revocation failed: {revoke}"));
        }
        let drain = services.drain(id, generation, 250);
        if drain != 0 {
            return Err(format!("runtime service drain incomplete: {drain}"));
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    };
    struct Services {
        events: Mutex<Vec<(String, u64)>>,
        drain: AtomicI32,
    }
    impl super::super::native_plugin_abi::NativePluginServiceLifetime for Services {
        unsafe fn query_service(
            &self,
            _: &str,
            _: u64,
            _: &str,
            _: u32,
            _: u32,
            _: *mut std::ffi::c_void,
        ) -> i32 {
            3
        }
        fn revoke(&self, id: &str, generation: u64) -> i32 {
            self.events.lock().unwrap().push((id.into(), generation));
            0
        }
        fn drain(&self, _: &str, _: u64, _: u32) -> i32 {
            self.drain.load(Ordering::Relaxed)
        }
    }
    #[test]
    fn lua_scope_reuses_generation_and_retries_service_drain_without_losing_ownership() {
        let fixture = super::super::asset_tests::Fixture::with_capabilities(&[
            "runtime",
            "runtime-map",
            "runtime-overlay",
        ]);
        let state = fixture.lua.app_data_ref::<AppState>().unwrap();
        let services = Arc::new(Services {
            events: Mutex::default(),
            drain: AtomicI32::new(6),
        });
        *state.native_plugin_services.borrow_mut() = services.clone();
        state.set_runtime_mod_active("fixture", true);
        let generation = state.runtime_package_generation("fixture").unwrap();
        state.set_runtime_mod_active("fixture", true);
        assert_eq!(
            state.runtime_package_generation("fixture"),
            Some(generation)
        );
        state.retire_runtime_services("fixture").unwrap();
        assert!(
            services.events.lock().unwrap().is_empty(),
            "unused APIs must not create service tombstones"
        );
        state.mark_runtime_services_used("fixture");
        assert!(state.retire_runtime_services("fixture").is_err());
        assert_eq!(
            state.runtime_package_generation("fixture"),
            Some(generation)
        );
        services.drain.store(0, Ordering::Relaxed);
        state.retire_runtime_services("fixture").unwrap();
        assert!(
            services
                .events
                .lock()
                .unwrap()
                .iter()
                .all(|event| event == &("fixture".into(), generation))
        );
        state.set_runtime_mod_active("fixture", false);
        state.set_runtime_mod_active("fixture", true);
        assert_ne!(
            state.runtime_package_generation("fixture"),
            Some(generation)
        );
    }
    #[test]
    fn mixed_package_uses_native_activation_generation_for_lua() {
        let fixture = super::super::asset_tests::Fixture::with_capabilities(&[
            "runtime",
            "runtime-register-dll",
        ]);
        let state = fixture.lua.app_data_ref::<AppState>().unwrap();
        let probe = Arc::new(Mutex::new(super::super::NativeLifecycleTestState::default()));
        state.attach_native_lifecycle_fixture("fixture", probe);
        let generation = state.native_package_generation("fixture").unwrap();
        state.set_runtime_mod_active("fixture", true);
        assert_eq!(
            state.runtime_package_generation("fixture"),
            Some(generation)
        );
    }
}
