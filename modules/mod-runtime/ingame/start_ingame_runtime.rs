use super::IngameRuntime;
use crate::{
    RunArgs, RunOptions, RuntimePhase, ShroudForgeApi, cache::CacheDiff, env::AppState,
    runner::LuaModRunner,
};
use mod_loader::ModEnvironment;

impl IngameRuntime {
    pub fn start(
        env: &ModEnvironment,
        api: ShroudForgeApi,
        file_name: impl Into<String>,
    ) -> anyhow::Result<Self> {
        Self::start_with_schema(env, Some(api), file_name.into())
    }

    pub fn start_live(env: &ModEnvironment, file_name: impl Into<String>) -> anyhow::Result<Self> {
        Self::start_with_schema(env, None, file_name.into())
    }

    pub(super) fn start_with_schema(
        env: &ModEnvironment,
        api: Option<ShroudForgeApi>,
        file_name: String,
    ) -> anyhow::Result<Self> {
        tracing::debug!(target: "shroudforge::runtime", stage = "loader-config", "Runtime startup stage started");
        let loader_config =
            shroudforge_loader_config::config::read_loader(env.game_dir().as_std_path())
                .map_err(anyhow::Error::msg)?;
        tracing::debug!(target: "shroudforge::runtime", stage = "app-state", "Runtime startup stage started");
        let export_enabled = loader_config
            .pointer("/exports/enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let mut diagnostics =
            shroudforge_runtime_diagnostics::Session::new(env.game_dir().as_std_path()).with_background_io();
        let file_name: String = file_name.into();
        let state = AppState::new(
            env.clone(),
            api.clone(),
            RunArgs {
                file_name: file_name.clone(),
                options: RunOptions {
                    skip_cache: false,
                    force_patch: false,
                    is_server: None,
                    patch: false,
                    export: export_enabled,
                    export_dir: None,
                    phase: RuntimePhase::Ingame,
                },
            },
            &CacheDiff::new_dirty(),
        );
        let state = state.map_err(|_| anyhow::anyhow!("failed to initialize ShroudForge API"))?;
        tracing::debug!(target: "shroudforge::runtime", stage = "app-state", "Runtime startup stage completed");
        tracing::debug!(target: "shroudforge::runtime", stage = "lua-runner", "Runtime startup stage started");
        let runner = LuaModRunner::new(state)?;
        tracing::debug!(target: "shroudforge::runtime", stage = "lua-runner", "Runtime startup stage completed");
        let is_server = runner.lua.app_data_ref::<AppState>().unwrap().is_server();
        let root = env.game_dir().as_std_path().to_path_buf();
        if loader_config
            .pointer("/modules/network/enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
        {
            let (authorized_clients, restrict_network_clients) =
                shroudforge_steam_networking::read_network_allowlist(&loader_config);
            shroudforge_steam_networking::service_tick(
                &authorized_clients,
                restrict_network_clients,
            );
        }
        let (plan, mut errors) =
            super::plan_runtime_mods::plan_runtime_mods(env, &root, is_server, &loader_config);
        let loaded = plan
            .iter()
            .map(|item| {
                let bytes = serde_json::to_vec(item.info()).unwrap_or_default();
                (
                    item.info().id.clone(),
                    serde_json::json!(shroudforge_loader_config::config::revision(&bytes)),
                )
            })
            .collect();
        tracing::debug!(target: "shroudforge::runtime", stage = "runner-setup", planned_mods = plan.len(), "Runtime startup stage started");
        runner.setup(plan)?;
        tracing::debug!(target: "shroudforge::runtime", stage = "runner-setup", "Runtime startup stage completed");
        super::load_native_plugins::load_native_plugins(env, &runner, &mut errors);
        let lifecycle = super::load_runtime_callbacks::load_runtime_callbacks(
            env,
            &runner,
            &mut diagnostics,
            &mut errors,
        )?;
        let world_editor_enabled = loader_config
            .pointer("/modules/worldEditor/enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let configurations = env
            .mod_registry()
            .values()
            .map(|item| {
                let mut configuration =
                    serde_json::to_value(item.info()).expect("manifest is serializable");
                if item.info().id == "world-editor" && !world_editor_enabled {
                    configuration["enabled"] = false.into();
                }
                (
                    item.info().id.clone(),
                    item.fs().root().as_std_path().to_path_buf(),
                    configuration,
                )
            })
            .collect();
        let runtime = Self {
            diagnostics,
            runner,
            lifecycle,
            root,
            api,
            file_name,
            loaded,
            errors,
            configurations,
            observed: Default::default(),
            package_revisions: super::watch_package_revisions::PackageRevisions::start(),
            next_status: std::time::Instant::now(),
            processed_reload_request: None,
            publish_on_drop: true,
        };
        tracing::debug!(target: "shroudforge::runtime", stage = "status-publication", "Runtime startup stage started");
        runtime.publish_status(true);
        tracing::debug!(target: "shroudforge::runtime", stage = "status-publication", "Runtime startup stage completed");
        Ok(runtime)
    }
}
