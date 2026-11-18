use crate::{env::AppState, runner::LuaModRunner};
use mod_loader::ModEnvironment;

pub(super) fn load_native_plugins(
    env: &ModEnvironment,
    runner: &LuaModRunner,
    errors: &mut serde_json::Map<String, serde_json::Value>,
) {
    let state = runner.lua.app_data_ref::<AppState>().unwrap();
    // Usually prepared before the game entrypoint. This also supports callers
    // that create an in-game runtime directly and reuses process-owned handles.
    if let Err(error) =
        crate::native_dlls::apply_plan(env.game_dir().as_std_path(), state.is_server(), false)
    {
        tracing::error!(target: "shroudforge::dll_manager", %error, "DLL Manager could not prepare native DLLs");
        state.report_runtime_effect("dll-manager", "error", &error);
        errors.insert("dll-manager".into(), error.into());
    }

    match crate::native_dlls::deferred_native_plugins(
        env.game_dir().as_std_path(),
        state.is_server(),
    ) {
        Ok(plugins) => {
            for plugin in plugins {
                let entry = &plugin.entry;
                let Some(mod_id) = entry.mod_id.as_deref() else {
                    let error = "Lifecycle-managed native plugins require a manifest-backed runtime package";
                    state.report_runtime_effect(&entry.status_id, "error", error);
                    errors.insert(entry.status_id.clone(), error.into());
                    continue;
                };
                let Some(target_mod) = env.mod_registry().get(mod_id) else {
                    let error = "Lifecycle-managed native plugin is unavailable in the current runtime plan";
                    state.report_runtime_effect(mod_id, "error", error);
                    errors.insert(mod_id.to_owned(), error.into());
                    continue;
                };
                match state.load_native_dll_declaration(target_mod, &plugin) {
                    Ok(true) => {
                        tracing::debug!(target: "shroudforge::runtime", mod_id,
                            "Authorized native plugin queued for lifecycle initialization");
                        state.report_runtime_effect(
                            mod_id,
                            "loading",
                            "DLL Manager selected this ShroudForge lifecycle plugin; Lua runtime continues independently",
                        );
                    }
                    Ok(false) => {
                        state.report_runtime_effect(
                            mod_id,
                            "disabled",
                            "ShroudForge native plugin is disabled in native-plugin.ini",
                        );
                    }
                    Err(error) => {
                        tracing::error!(target: "shroudforge::runtime", mod_id,
                            "Lifecycle-managed native plugin was not queued: {error}");
                        state.report_runtime_effect(mod_id, "error", &error);
                        errors.insert(
                            mod_id.to_owned(),
                            format!("native plugin load failed: {error}").into(),
                        );
                    }
                }
            }
        }
        Err(error) => {
            tracing::error!(target: "shroudforge::dll_manager", %error,
                "DLL Manager lifecycle selections changed after startup");
            state.report_runtime_effect("dll-manager", "error", &error);
            errors.insert("dll-manager".into(), error.into());
        }
    }

    state.poll_native_dll_loads();
    for status in crate::native_dlls::statuses() {
        if status.state == "error" {
            errors.insert(status.id, status.detail.into());
        }
    }
}
