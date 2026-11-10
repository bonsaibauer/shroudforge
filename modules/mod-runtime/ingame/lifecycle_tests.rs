use super::{Lifecycle, settings_only_manifest_change};
use serde_json::json;

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
