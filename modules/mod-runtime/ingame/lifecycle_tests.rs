use super::{Lifecycle, settings_only_manifest_change};
use serde_json::json;

fn native_package_runtime(
    pure_native: bool,
    retry_lua_cleanup: bool,
) -> (
    crate::session::asset_tests::Fixture,
    super::IngameRuntime,
    crate::session::NativeLifecycleProbe,
) {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let fixture = crate::session::asset_tests::Fixture::with_capabilities(&[
        "runtime",
        "runtime-register-dll",
    ]);
    if pure_native {
        std::fs::remove_file(fixture.root.join("mods/fixture/src/mod.lua")).unwrap();
    }
    let state = fixture
        .lua
        .remove_app_data::<crate::session::AppState>()
        .unwrap();
    let env = state.env().clone();
    let probe = Arc::new(Mutex::new(
        crate::session::NativeLifecycleTestState::default(),
    ));
    state.attach_native_lifecycle_fixture("fixture", probe.clone());
    let runner = crate::runner::LuaModRunner::new(state).unwrap();
    runner
        .setup([env.mod_registry().get("fixture").unwrap()])
        .unwrap();
    let mut item = Lifecycle {
        id: "fixture".into(),
        load: None,
        update: None,
        unload: None,
        update_interval: std::time::Duration::from_millis(50),
        next_update: std::time::Instant::now(),
        active: false,
        cleanup_pending: false,
    };
    if !pure_native {
        let load_probe = probe.clone();
        item.load = Some(
            runner
                .lua
                .create_registry_value(
                    runner
                        .lua
                        .create_function(move |lua, ()| {
                            assert!(
                                lua.app_data_ref::<crate::session::AppState>()
                                    .unwrap()
                                    .runtime_mod_is_active("fixture")
                            );
                            load_probe.lock().unwrap().events.push("lua.load");
                            Ok(())
                        })
                        .unwrap(),
                )
                .unwrap(),
        );
        let update_probe = probe.clone();
        item.update = Some(
            runner
                .lua
                .create_registry_value(
                    runner
                        .lua
                        .create_function(move |_, _: f64| {
                            update_probe.lock().unwrap().events.push("lua.update");
                            Ok(())
                        })
                        .unwrap(),
                )
                .unwrap(),
        );
        let unload_probe = probe.clone();
        let unload_calls = AtomicUsize::new(0);
        item.unload = Some(
            runner
                .lua
                .create_registry_value(
                    runner
                        .lua
                        .create_function(move |lua, ()| {
                            assert!(
                                lua.app_data_ref::<crate::session::AppState>()
                                    .unwrap()
                                    .runtime_mod_is_active("fixture")
                            );
                            unload_probe.lock().unwrap().events.push("lua.unload");
                            if retry_lua_cleanup && unload_calls.fetch_add(1, Ordering::SeqCst) == 0
                            {
                                Err(mlua::Error::runtime("Scene gate is busy"))
                            } else {
                                Ok(())
                            }
                        })
                        .unwrap(),
                )
                .unwrap(),
        );
    }
    let root = fixture.root.clone();
    let runtime = super::IngameRuntime {
        diagnostics: shroudforge_runtime_diagnostics::Session::new(&root),
        root: root.clone(),
        api: None,
        file_name: "fixture".into(),
        loaded: Default::default(),
        errors: Default::default(),
        next_status: std::time::Instant::now(),
        configurations: Vec::new(),
        observed: Default::default(),
        package_revisions: super::watch_package_revisions::PackageRevisions::start(root),
        runner,
        lifecycle: vec![item],
        processed_reload_request: None,
        publish_on_drop: false,
    };
    (fixture, runtime, probe)
}

#[test]
fn lua_update_failure_skips_native_update_and_stops_the_same_package() {
    let (_fixture, mut runtime, probe) = native_package_runtime(false, false);
    runtime.lifecycle[0]
        .activate_runtime_mod(&runtime.runner.lua, None)
        .unwrap()
        .unwrap();
    runtime.lifecycle[0].update = Some(
        runtime
            .runner
            .lua
            .create_registry_value(
                runtime
                    .runner
                    .lua
                    .create_function(|_, _: f64| {
                        Err::<(), _>(mlua::Error::runtime("Lua update failed"))
                    })
                    .unwrap(),
            )
            .unwrap(),
    );
    runtime.update_mod_callbacks(0.016);
    assert_eq!(
        probe.lock().unwrap().events,
        [
            "lua.load",
            "native.load",
            "lua.unload",
            "native.stop",
            "native.shutdown"
        ]
    );
    assert!(runtime.active_mod_ids().is_empty());
    assert!(
        runtime.errors["fixture"]
            .as_str()
            .unwrap()
            .contains("Lua update failed")
    );
}

#[test]
fn common_traversal_preserves_native_ticks_and_the_lua_interval() {
    let (_fixture, mut runtime, probe) = native_package_runtime(false, false);
    runtime.lifecycle[0]
        .activate_runtime_mod(&runtime.runner.lua, None)
        .unwrap()
        .unwrap();
    runtime.lifecycle[0].next_update =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    runtime.update_mod_callbacks(0.016);
    runtime.update_mod_callbacks(0.016);
    assert_eq!(
        probe.lock().unwrap().events,
        ["lua.load", "native.load", "native.update", "native.update"]
    );
    runtime.lifecycle[0].next_update =
        std::time::Instant::now() - std::time::Duration::from_secs(5);
    runtime.update_mod_callbacks(0.016);
    assert_eq!(
        probe.lock().unwrap().events,
        [
            "lua.load",
            "native.load",
            "native.update",
            "native.update",
            "lua.update",
            "native.update"
        ]
    );
    assert!(runtime.lifecycle[0].next_update > std::time::Instant::now());
}

#[test]
fn native_update_failure_unloads_the_whole_package_once() {
    let (_fixture, mut runtime, probe) = native_package_runtime(false, false);
    runtime.lifecycle[0]
        .activate_runtime_mod(&runtime.runner.lua, None)
        .unwrap()
        .unwrap();
    probe.lock().unwrap().fail_update = true;
    runtime.update_mod_callbacks(0.016);
    assert_eq!(
        probe.lock().unwrap().events,
        [
            "lua.load",
            "native.load",
            "lua.update",
            "native.update",
            "lua.unload",
            "native.stop",
            "native.shutdown"
        ]
    );
    assert!(runtime.active_mod_ids().is_empty());
    assert!(
        !runtime
            .runner
            .lua
            .app_data_ref::<crate::session::AppState>()
            .unwrap()
            .runtime_mod_is_active("fixture")
    );
    runtime.update_mod_callbacks(0.016);
    assert_eq!(probe.lock().unwrap().events.len(), 7);
    assert!(
        runtime.errors["fixture"]
            .as_str()
            .unwrap()
            .contains("native update failed")
    );
}

#[test]
fn cleanup_retry_never_runs_another_native_update() {
    let (_fixture, mut runtime, probe) = native_package_runtime(false, true);
    runtime.lifecycle[0]
        .activate_runtime_mod(&runtime.runner.lua, None)
        .unwrap()
        .unwrap();
    probe.lock().unwrap().fail_update = true;
    runtime.update_mod_callbacks(0.016);
    assert!(runtime.lifecycle[0].cleanup_pending && runtime.lifecycle[0].active);
    assert!(!probe.lock().unwrap().events.contains(&"native.stop"));
    runtime.lifecycle[0].next_update = std::time::Instant::now();
    runtime.update_mod_callbacks(0.016);
    assert!(!runtime.lifecycle[0].cleanup_pending && !runtime.lifecycle[0].active);
    assert_eq!(
        probe.lock().unwrap().events,
        [
            "lua.load",
            "native.load",
            "lua.update",
            "native.update",
            "lua.unload",
            "lua.unload",
            "native.stop",
            "native.shutdown"
        ]
    );
}

#[test]
fn native_start_failure_rolls_back_the_same_lua_scope() {
    let (_fixture, mut runtime, probe) = native_package_runtime(false, false);
    probe.lock().unwrap().fail_initialize = true;
    assert!(
        runtime.lifecycle[0]
            .activate_runtime_mod(&runtime.runner.lua, None)
            .unwrap()
            .is_err()
    );
    runtime.lifecycle[0]
        .rollback_failed_activation(&runtime.runner.lua)
        .unwrap();
    assert_eq!(
        probe.lock().unwrap().events,
        [
            "lua.load",
            "native.load",
            "native.stop",
            "native.shutdown",
            "lua.unload"
        ]
    );
    assert!(runtime.active_mod_ids().is_empty());
    assert!(
        !runtime
            .runner
            .lua
            .app_data_ref::<crate::session::AppState>()
            .unwrap()
            .runtime_mod_is_active("fixture")
    );
}

#[test]
fn native_cleanup_failure_keeps_the_common_scope_and_blocks_updates() {
    let (_fixture, mut runtime, probe) = native_package_runtime(false, false);
    runtime.lifecycle[0]
        .activate_runtime_mod(&runtime.runner.lua, None)
        .unwrap()
        .unwrap();
    {
        let mut state = probe.lock().unwrap();
        state.fail_update = true;
        state.fail_shutdown = true;
    }
    runtime.update_mod_callbacks(0.016);
    assert!(runtime.lifecycle[0].cleanup_pending && runtime.lifecycle[0].active);
    assert!(runtime.active_mod_ids().is_empty());
    runtime.lifecycle[0].next_update = std::time::Instant::now();
    runtime.update_mod_callbacks(0.016);
    assert_eq!(
        probe
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|event| **event == "native.update")
            .count(),
        1
    );
    let state = runtime
        .runner
        .lua
        .app_data_ref::<crate::session::AppState>()
        .unwrap();
    assert!(state.runtime_mod_is_active("fixture"));
    assert!(state.activate_native_plugin("fixture").is_err());
}

#[test]
fn native_only_package_uses_the_common_loader_without_a_lua_stub() {
    let (fixture, mut runtime, probe) = native_package_runtime(true, false);
    assert!(!fixture.root.join("mods/fixture/src/mod.lua").exists());
    assert!(probe.lock().unwrap().events.is_empty());
    let env = runtime
        .runner
        .lua
        .app_data_ref::<crate::session::AppState>()
        .unwrap()
        .env()
        .clone();
    runtime.lifecycle = super::load_runtime_callbacks::load_runtime_callbacks(
        &env,
        &runtime.runner,
        &mut runtime.diagnostics,
        &mut runtime.errors,
    )
    .unwrap();
    assert_eq!(
        runtime.active_mod_ids(),
        ["fixture"],
        "{:?}",
        runtime.errors
    );
    runtime.update_mod_callbacks(0.016);
    assert_eq!(
        probe.lock().unwrap().events,
        ["native.load", "native.update"]
    );
    runtime
        .unload_runtime_packages(&["fixture".into()])
        .unwrap();
    assert_eq!(
        probe.lock().unwrap().events,
        [
            "native.load",
            "native.update",
            "native.stop",
            "native.shutdown"
        ]
    );
}

#[test]
fn blocked_native_package_never_initializes_its_callbacks() {
    let (_fixture, mut runtime, probe) = native_package_runtime(true, false);
    let env = runtime
        .runner
        .lua
        .app_data_ref::<crate::session::AppState>()
        .unwrap()
        .env()
        .clone();
    runtime.errors.insert(
        "fixture".into(),
        "dependency-blocked: required package is unavailable".into(),
    );
    runtime.lifecycle = super::load_runtime_callbacks::load_runtime_callbacks(
        &env,
        &runtime.runner,
        &mut runtime.diagnostics,
        &mut runtime.errors,
    )
    .unwrap();
    assert!(runtime.active_mod_ids().is_empty());
    runtime.update_mod_callbacks(0.016);
    assert!(probe.lock().unwrap().events.is_empty());
    drop(runtime);
    assert!(probe.lock().unwrap().events.is_empty());
}

#[test]
fn failed_start_or_update_retains_cleanup_scope_until_unload_succeeds() -> mlua::Result<()> {
    for failed_callback in ["on_load", "on_update"] {
        let lua = mlua::Lua::new();
        let module: mlua::Table = lua
            .load(
                r#"
                value, cleanup_attempts = 1, 0
                return {
                    on_load=function() value = 2; error("startup failed after write") end,
                    on_update=function() value = 2; error("update failed after write") end,
                    on_unload=function()
                        cleanup_attempts = cleanup_attempts + 1
                        if cleanup_attempts == 1 then error("Scene gate is busy") end
                        value = 1
                    end,
                }
            "#,
            )
            .eval()?;
        let callback: mlua::Function = module.get(failed_callback)?;
        assert!(callback.call::<()>(()).is_err());
        let mut item = Lifecycle {
            id: "independent-mod".into(),
            load: None,
            update: None,
            unload: Some(lua.create_registry_value(module.get::<mlua::Function>("on_unload")?)?),
            update_interval: std::time::Duration::from_millis(100),
            next_update: std::time::Instant::now(),
            active: true,
            cleanup_pending: false,
        };
        assert!(item.try_unload(&lua).is_err());
        assert!(
            item.active && item.cleanup_pending,
            "busy cleanup retains its writable scope"
        );
        assert_eq!(lua.globals().get::<i64>("value")?, 2);
        item.try_unload(&lua)?;
        assert!(!item.active && !item.cleanup_pending);
        assert_eq!(lua.globals().get::<i64>("value")?, 1);
        assert_eq!(lua.globals().get::<i64>("cleanup_attempts")?, 2);
    }
    Ok(())
}

#[test]
fn setting_value_change_does_not_look_like_package_metadata_change() {
    let applied = json!({
        "enabled": true,
        "settingValues": {"maximumCopyableProps": 60000},
        "settings": [{
            "key": "maximumCopyableProps",
            "type": "number",
            "default": 60000,
            "label": "Maximum captured props"
        }]
    });
    let desired = json!({
        "enabled": true,
        "settingValues": {"maximumCopyableProps": 120000},
        "settings": [{
            "key": "maximumCopyableProps",
            "type": "number",
            "default": 60000,
            "label": "Maximum captured props"
        }]
    });

    assert!(settings_only_manifest_change(&desired, &applied));
}

#[test]
fn setting_definition_change_still_requires_restart() {
    let applied = json!({
        "enabled": true,
        "settingValues": {"maximumCopyableProps": 60000},
        "settings": [{
            "key": "maximumCopyableProps",
            "type": "number",
            "default": 60000,
            "label": "Maximum captured props"
        }]
    });
    let desired = json!({
        "enabled": true,
        "settingValues": {"maximumCopyableProps": 120000},
        "settings": [{
            "key": "maximumCopyableProps",
            "type": "number",
            "default": 120000,
            "label": "Maximum captured props (changed)"
        }]
    });

    assert!(!settings_only_manifest_change(&desired, &applied));
}

#[test]
fn hybrid_mod_applies_live_settings_without_applying_restart_settings() {
    let mut manifest: mod_loader::ModManifest = serde_json::from_value(json!({
        "id": "hybrid-mod", "name": "Hybrid", "version": "1.0.0",
        "capabilities": ["patch", "runtime"],
        "settingValues": {"liveValue": 5, "startupValue": 10},
        "settings": [
            {"key": "liveValue", "type": "integer", "label": "Live", "default": 3,
             "applyAt": "live", "restartRequired": false},
            {"key": "startupValue", "type": "integer", "label": "Startup", "default": 10,
             "applyAt": "restart", "restartRequired": true}
        ]
    }))
    .unwrap();
    let applied = json!({"liveValue": 3, "startupValue": 10});
    let apply_live = super::refresh_runtime_configuration::settings_to_apply_live;
    assert_eq!(
        apply_live(&manifest, &applied),
        Some(json!({"liveValue": 5, "startupValue": 10}))
    );
    manifest.setting_values["startupValue"] = 20.into();
    assert_eq!(
        apply_live(&manifest, &applied),
        Some(json!({"liveValue": 5, "startupValue": 10}))
    );
    manifest.setting_values["startupValue"] = 10.into();
    manifest.settings[0].restart_required = true;
    assert_eq!(
        apply_live(&manifest, &applied),
        Some(json!({"liveValue": 3, "startupValue": 10}))
    );
}

#[test]
fn live_settings_preserve_runtime_only_behavior_and_require_runtime_capability() {
    let mut manifest: mod_loader::ModManifest = serde_json::from_value(json!({
        "id": "runtime-mod", "name": "Runtime", "version": "1.0.0",
        "capabilities": ["runtime"], "settingValues": {"value": 2},
        "settings": [{"key": "value", "type": "integer", "label": "Value", "default": 1}]
    }))
    .unwrap();
    let applied = json!({"value": 1});
    let apply_live = super::refresh_runtime_configuration::settings_to_apply_live;
    assert_eq!(apply_live(&manifest, &applied), Some(json!({"value": 2})));
    manifest.capabilities = vec![mod_loader::Capability::Patch];
    manifest.settings[0].apply_at = "live".into();
    assert_eq!(apply_live(&manifest, &applied), None);
    manifest
        .capabilities
        .push(mod_loader::Capability::RuntimeRegisterDll);
    assert_eq!(apply_live(&manifest, &applied), Some(json!({"value": 2})));
}

#[test]
fn padlock_change_does_not_look_like_package_metadata_change() {
    let applied = json!({
        "enabled": true,
        "settingValues": {"keyMark": "SDL:30"},
        "settings": [{
            "key": "keyMark",
            "type": "string",
            "control": "keybind",
            "default": "SDL:30",
            "label": "Start point",
            "reserve": {"when": "hammer", "keyboard": true, "mouse": false}
        }]
    });
    let mut desired = applied.clone();
    desired["settings"][0]["reserved"] = json!({"keyboard": false});
    assert!(settings_only_manifest_change(&desired, &applied));

    desired["settings"][0]["reserve"]["when"] = json!("other");
    assert!(!settings_only_manifest_change(&desired, &applied));
}
