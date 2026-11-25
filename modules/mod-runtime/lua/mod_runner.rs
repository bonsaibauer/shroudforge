use std::rc::Rc;

use mod_loader::{Capability, Mod, ModManifest};
use tracing::debug;

use crate::{
    env::{AppFeatures, AppState},
    lua::{FunctionArgs, LuaVM},
    runner::state::{RunnerLocalState, RunnerState},
};

pub(crate) use super::require_mod as state;

pub struct LuaModRunner {
    pub(crate) lua: LuaVM,
    state: Rc<RunnerState>,
}

impl LuaModRunner {
    /// The job has its own globals while retaining the real mod's bound APIs.
    pub(crate) fn diagnostic_environment(&self, mod_id: &str) -> mlua::Result<mlua::Table> {
        let source = self.state.environment(mod_id).ok_or_else(|| {
            mlua::Error::runtime(format!("diagnostic API owner is not loaded: {mod_id}"))
        })?;
        let env = self.lua.create_table()?;
        for entry in source.pairs::<mlua::Value, mlua::Value>() {
            let (key, value) = entry?;
            if key
                .as_string()
                .is_some_and(|name| name.as_bytes().as_ref() == b"_G")
            {
                continue;
            }
            env.raw_set(key, value)?;
        }
        env.raw_set("_G", env.clone())?;
        Ok(env)
    }

    pub fn new(context: AppState) -> mlua::Result<Self> {
        let lua = LuaVM::new()?;
        let state = Rc::new(RunnerState::default());

        lua.set_app_data(context);

        Ok(Self { lua, state })
    }

    pub fn run(&self) -> mlua::Result<()> {
        let app_state = self.lua.app_data_ref::<AppState>().unwrap();

        for r#mod in self.state.mods().values() {
            // Runtime mods may export files during play. Export permission does
            // not make their entrypoints safe to execute during asset preparation.
            if !r#mod.info().requires_pregame() {
                continue;
            }
            let has_patch_capability = r#mod.info().capabilities.contains(&Capability::Patch);
            let has_export_capability = r#mod.info().capabilities.contains(&Capability::Export);
            let should_run = has_patch_capability && app_state.has_feature(AppFeatures::PATCH)
                || has_export_capability && app_state.has_feature(AppFeatures::EXPORT);

            if !should_run {
                continue;
            }

            debug!(
                mod_id = r#mod.info().id,
                mod_name = r#mod.info().name,
                "Running mod",
            );

            self.state.require_mod(&r#mod.info().id, &self.lua)?;
        }

        Ok(())
    }

    pub(crate) fn runtime_mod_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for r#mod in self.state.mods().values() {
            if !r#mod
                .info()
                .capabilities
                .iter()
                .any(|capability| capability.requires_runtime())
            {
                continue;
            }
            ids.push(r#mod.info().id.clone());
        }
        ids
    }

    pub(crate) fn load_runtime_module(&self, id: &str) -> mlua::Result<crate::lua::LuaValue> {
        let mods = self.state.mods();
        let target = mods
            .get(id)
            .ok_or_else(|| mlua::Error::runtime("runtime package is not loaded"))?;
        let has_lua = target.fs().exists("src/mod.lua");
        drop(mods);
        if !has_lua
            && self
                .lua
                .app_data_ref::<AppState>()
                .unwrap()
                .native_plugin_has_callbacks(id)
        {
            return Ok(mlua::Value::Nil);
        }
        self.state.require_mod(id, &self.lua)
    }

    pub fn setup<'a>(&self, mods: impl IntoIterator<Item = &'a Mod>) -> mlua::Result<()> {
        for r#mod in mods {
            tracing::debug!(target: "shroudforge::runtime", stage = "runner-setup",
                mod_id = %r#mod.info().id, "Preparing runtime mod environment");
            self.setup_mod(r#mod)?;
            tracing::debug!(target: "shroudforge::runtime", stage = "runner-setup",
                mod_id = %r#mod.info().id, "Runtime mod environment ready");
        }

        Ok(())
    }

    fn setup_mod(&self, r#mod: &Mod) -> mlua::Result<()> {
        let env = self.create_mod_environment(r#mod.info())?;

        crate::env::register(&self.lua, &env, r#mod)?;

        self.state.add_mod(r#mod.clone(), env);

        Ok(())
    }

    fn create_mod_environment(&self, mod_info: &ModManifest) -> mlua::Result<mlua::Table> {
        let env = self.lua.create_base_environment()?;
        let local_state = RunnerLocalState::new(&mod_info.id, self.state.clone());

        env.raw_set(
            "require",
            self.lua.create_function(move |lua, args: FunctionArgs| {
                let id = args.get::<String>(0)?;

                local_state.require(&id, lua)
            })?,
        )?;

        Ok(env)
    }
}
