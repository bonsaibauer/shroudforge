use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use mlua::Table;
use mod_loader::{FileSystem, Mod, ModManifest};
use parking_lot::MutexGuard;

use crate::{
    lua::{LuaError, LuaValue},
    util::RequirePath,
};

pub struct ModLuaState {
    r#mod: Mod,
    env: Table,
}

impl ModLuaState {
    #[inline]
    pub fn info(&self) -> &ModManifest {
        self.r#mod.info()
    }

    #[inline]
    pub fn fs(&self) -> MutexGuard<'_, FileSystem> {
        self.r#mod.fs()
    }
}

#[derive(Default)]
pub struct RunnerState {
    mods: RefCell<indexmap::IndexMap<String, ModLuaState>>,
    loaded: RefCell<HashMap<String, LuaValue>>,
    is_loading: RefCell<HashSet<String>>,
}

struct LoadingGuard<'a> {
    entries: &'a RefCell<HashSet<String>>,
    key: String,
}
impl Drop for LoadingGuard<'_> {
    fn drop(&mut self) {
        self.entries.borrow_mut().remove(&self.key);
    }
}

impl RunnerState {
    #[inline]
    pub fn mods(&self) -> Ref<'_, indexmap::IndexMap<String, ModLuaState>> {
        self.mods.borrow()
    }

    pub fn add_mod(&self, r#mod: Mod, env: Table) {
        let id = r#mod.info().id.clone();
        let state = ModLuaState { r#mod, env };

        self.mods.borrow_mut().insert(id, state);
    }

    pub fn require(&self, qualified_id: &str, lua: &mlua::Lua) -> mlua::Result<LuaValue> {
        if let Some(table) = self.loaded.borrow().get(qualified_id) {
            return Ok(table.clone());
        }

        // check for circular dependencies
        if self.is_loading.borrow().contains(qualified_id) {
            return Err(LuaError::circular_dependency(qualified_id));
        }

        self.is_loading
            .borrow_mut()
            .insert(qualified_id.to_string());
        let _loading = LoadingGuard {
            entries: &self.is_loading,
            key: qualified_id.to_owned(),
        };

        // load the module
        let require_path = RequirePath::parse_qualified(qualified_id)?;

        let func = {
            let mods = self.mods.borrow();
            let mod_state = mods
                .get(require_path.mod_id())
                .ok_or_else(|| LuaError::mod_not_found(require_path.mod_id()))?;

            let mut path = String::new();

            if !require_path.path().is_empty() {
                path.push_str("src");

                for segment in require_path.path().split('.') {
                    path.push('/');
                    path.push_str(segment);
                }

                path.push_str(".lua");
            }

            let mut fs = mod_state.fs();
            let reader = fs
                .read_file(&path)
                .map_err(|e| LuaError::module_load(&path, e))?;
            let chunk =
                std::io::read_to_string(reader).map_err(|e| LuaError::module_load(&path, e))?;

            drop(fs);

            lua.load(chunk)
                .set_name(qualified_id)
                .set_environment(mod_state.env.clone())
                .into_function()?
        };
        let result = func.call::<LuaValue>(());
        self.is_loading.borrow_mut().remove(qualified_id);
        let result = result?;

        // register mod as loaded
        let mut loaded_mut = self.loaded.borrow_mut();

        loaded_mut.insert(qualified_id.to_string(), result.clone());

        if require_path.path() == "mod" {
            loaded_mut.insert(format!("mods.{}", require_path.mod_id()), result.clone());
        }

        Ok(result)
    }

    /// Loads a package entrypoint by its exact manifest id. Unlike a dotted
    /// module path, ids such as `mod.example` are not split into path segments.
    pub fn require_mod(&self, mod_id: &str, lua: &mlua::Lua) -> mlua::Result<LuaValue> {
        let key = format!("mods:{mod_id}");
        if let Some(value) = self.loaded.borrow().get(&key) {
            return Ok(value.clone());
        }
        if !self.is_loading.borrow_mut().insert(key.clone()) {
            return Err(LuaError::circular_dependency(mod_id));
        }
        let _loading = LoadingGuard {
            entries: &self.is_loading,
            key: key.clone(),
        };
        let function = {
            let mods = self.mods.borrow();
            let state = mods
                .get(mod_id)
                .ok_or_else(|| LuaError::mod_not_found(mod_id))?;
            let mut fs = state.fs();
            let reader = fs
                .read_file("src/mod.lua")
                .map_err(|error| LuaError::module_load("src/mod.lua", error))?;
            let source = std::io::read_to_string(reader)
                .map_err(|error| LuaError::module_load("src/mod.lua", error))?;
            drop(fs);
            lua.load(source)
                .set_name(mod_id)
                .set_environment(state.env.clone())
                .into_function()?
        };
        let value = function.call::<LuaValue>(());
        self.is_loading.borrow_mut().remove(&key);
        let value = value?;
        self.loaded.borrow_mut().insert(key.clone(), value.clone());
        Ok(value)
    }
}

pub struct RunnerLocalState {
    mod_id: String,
    loaded: RefCell<HashMap<String, LuaValue>>,
    global_state: Rc<RunnerState>,
}

impl RunnerLocalState {
    #[inline]
    pub fn new<S: AsRef<str>>(mod_id: S, global_state: Rc<RunnerState>) -> Self {
        Self {
            mod_id: mod_id.as_ref().to_string(),
            loaded: RefCell::new(HashMap::new()),
            global_state,
        }
    }

    pub fn require(&self, id: &str, lua: &mlua::Lua) -> mlua::Result<LuaValue> {
        // lookup local cache first
        if let Some(table) = self.loaded.borrow().get(id) {
            return Ok(table.clone());
        }

        // lookup global cache (mods.id.path)
        if let Some(table) = self.global_state.loaded.borrow().get(id) {
            return Ok(table.clone());
        }

        // load the module
        let require_path = RequirePath::parse(id, &self.mod_id)?;
        let qualified_id = require_path.qualified_id();

        let result = self.global_state.require(&qualified_id, lua)?;

        // register mod as loaded in local state
        if require_path.is_local() {
            self.loaded
                .borrow_mut()
                .insert(require_path.path().to_string(), result.clone());
        }

        Ok(result)
    }
}
