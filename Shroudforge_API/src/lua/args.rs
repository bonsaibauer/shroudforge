use crate::lua::{CastLua, CastLuaExt, LuaError, LuaValue};

#[allow(unused)]
pub trait Args {
    fn get<'a, T>(&'a self, index: usize) -> mlua::Result<T::Output>
    where
        T: CastLua<'a>;
}

#[derive(Default, Debug, Clone)]
#[repr(transparent)]
pub struct FunctionArgs(mlua::MultiValue);

impl FunctionArgs {
    pub fn get<'a, T>(&'a self, index: usize) -> mlua::Result<T::Output>
    where
        T: CastLua<'a>,
    {
        let value = self.0.get(index).unwrap_or(&LuaValue::Nil);

        value
            .try_cast::<T>()?
            .ok_or_else(|| LuaError::bad_argument_type(index, T::name(), value))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Args for FunctionArgs {
    fn get<'a, T>(&'a self, index: usize) -> mlua::Result<T::Output>
    where
        T: CastLua<'a>,
    {
        Self::get::<T>(self, index)
    }
}

impl mlua::FromLuaMulti for FunctionArgs {
    fn from_lua_multi(multi: mlua::MultiValue, _lua: &mlua::Lua) -> mlua::Result<Self> {
        Ok(Self(multi))
    }
}

impl mlua::IntoLuaMulti for FunctionArgs {
    fn into_lua_multi(self, _lua: &mlua::Lua) -> mlua::Result<mlua::MultiValue> {
        Ok(self.0)
    }
}

#[derive(Default, Debug, Clone)]
#[repr(transparent)]
pub struct MethodArgs(mlua::MultiValue);

impl MethodArgs {
    pub fn this<'a, T>(&'a self) -> mlua::Result<T::Output>
    where
        T: CastLua<'a>,
    {
        let value = self
            .0
            .front()
            .ok_or_else(|| LuaError::bad_method_call(T::name(), &LuaValue::Nil))?;

        value
            .try_cast::<T>()?
            .ok_or_else(|| LuaError::bad_method_call(T::name(), value))
    }

    pub fn get<'a, T>(&'a self, index: usize) -> mlua::Result<T::Output>
    where
        T: CastLua<'a>,
    {
        let value = self.0.get(index + 1).unwrap_or(&LuaValue::Nil);

        value
            .try_cast::<T>()?
            .ok_or_else(|| LuaError::bad_argument_type(index, T::name(), value))
    }
}

impl Args for MethodArgs {
    fn get<'a, T>(&'a self, index: usize) -> mlua::Result<T::Output>
    where
        T: CastLua<'a>,
    {
        Self::get::<T>(self, index)
    }
}

impl mlua::FromLuaMulti for MethodArgs {
    fn from_lua_multi(multi: mlua::MultiValue, _lua: &mlua::Lua) -> mlua::Result<Self> {
        Ok(Self(multi))
    }
}

impl mlua::IntoLuaMulti for MethodArgs {
    fn into_lua_multi(self, _lua: &mlua::Lua) -> mlua::Result<mlua::MultiValue> {
        Ok(self.0)
    }
}
