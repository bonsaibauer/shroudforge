use mlua::Table;

use crate::{
    log::{info, warn},
    lua::LuaValue,
};

pub fn register(lua: &mlua::Lua, env: &Table, mod_id: &str) -> mlua::Result<()> {
    env.raw_set(
        "print",
        lua.create_function({
            let mod_id = mod_id.to_string();

            move |_, args: mlua::Variadic<LuaValue>| {
                let mut output = String::new();

                for arg in args {
                    if !output.is_empty() {
                        output.push('\t');
                    }

                    output.push_str(&arg.to_string()?);
                }

                info!(mod_id = %mod_id, "{}", output);

                Ok(())
            }
        })?,
    )?;

    env.raw_set(
        "warn",
        lua.create_function({
            let mod_id = mod_id.to_string();

            move |_, args: mlua::Variadic<LuaValue>| {
                let mut output = String::new();

                for arg in args {
                    if !output.is_empty() {
                        output.push('\t');
                    }

                    output.push_str(&arg.to_string()?);
                }

                warn!(mod_id = %mod_id, "{}", output);

                Ok(())
            }
        })?,
    )?;

    Ok(())
}
