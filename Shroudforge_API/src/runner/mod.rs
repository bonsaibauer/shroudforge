use std::rc::Rc;

use mod_loader::{Capability, Mod, ModManifest};
use tracing::info;

use crate::{
    env::{AppFeatures, AppState},
    lua::{FunctionArgs, LuaVM},
    runner::state::{RunnerLocalState, RunnerState},
};

mod state;

pub struct LuaModRunner {
    pub(crate) lua: LuaVM,
    state: Rc<RunnerState>,
}

impl LuaModRunner {
    pub fn new(context: AppState) -> mlua::Result<Self> {
        let lua = LuaVM::new()?;
        let state = Rc::new(RunnerState::default());

        lua.set_app_data(context);

        Ok(Self { lua, state })
    }

    pub fn run(&self) -> mlua::Result<()> {
        let app_state = self.lua.app_data_ref::<AppState>().unwrap();

        for r#mod in self.state.mods().values() {
            let has_assets_write_capability =
                r#mod.info().capabilities.contains(&Capability::AssetsWrite);
            let has_export_capability = r#mod.info().capabilities.contains(&Capability::Export);
            let should_run = has_assets_write_capability
                && app_state.has_feature(AppFeatures::ASSETS_WRITE)
                || has_export_capability && app_state.has_feature(AppFeatures::EXPORT);

            if !should_run {
                continue;
            }

            info!(
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
            if !r#mod.info().capabilities.contains(&Capability::Runtime) {
                continue;
            }
            ids.push(r#mod.info().id.clone());
        }
        ids.sort();
        ids
    }

    pub(crate) fn load_runtime_module(&self, id: &str) -> mlua::Result<crate::lua::LuaValue> {
        self.state.require_mod(id, &self.lua)
    }

    pub fn setup<'a>(&self, mods: impl IntoIterator<Item = &'a Mod>) -> mlua::Result<()> {
        for r#mod in mods {
            self.setup_mod(r#mod)?;
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
