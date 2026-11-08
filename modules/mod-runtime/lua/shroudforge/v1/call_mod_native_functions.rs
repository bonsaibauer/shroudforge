use mlua::{Lua, MultiValue, Table, Value, Variadic};

use crate::{
    env::Buffer,
    native_bridge::{self, NativeArgument, NativeResult},
};

/// `shroudforge.native`: the functions this mod's own DLLs registered.
/// The mod id is bound here, so a mod cannot reach another mod's functions.
pub(crate) fn create(lua: &Lua, mod_id: &str) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let owner = mod_id.to_owned();
    table.raw_set(
        "functions",
        lua.create_function(move |lua, ()| {
            lua.create_sequence_from(native_bridge::function_names(&owner))
        })?,
    )?;
    let owner = mod_id.to_owned();
    table.raw_set(
        "call",
        lua.create_function(move |lua, (name, arguments): (String, Variadic<Value>)| {
            call(lua, &owner, &name, &arguments)
        })?,
    )?;
    Ok(table)
}

fn call(lua: &Lua, mod_id: &str, name: &str, arguments: &[Value]) -> mlua::Result<MultiValue> {
    // Strings and buffers stay borrowed, not copied, until the native call returns.
    let strings: Vec<_> = arguments
        .iter()
        .filter_map(|value| value.as_string().map(|text| text.as_bytes()))
        .collect();
    let buffers = arguments
        .iter()
        .filter_map(|value| match value {
            Value::UserData(data) => Some(data.borrow::<Buffer>().map_err(|_| {
                mlua::Error::runtime("shroudforge.native.call accepts only buffers as userdata")
            })),
            _ => None,
        })
        .collect::<mlua::Result<Vec<_>>>()?;
    let (mut next_string, mut next_buffer) = (strings.iter(), buffers.iter());
    let mut native = Vec::with_capacity(arguments.len());
    for (index, value) in arguments.iter().enumerate() {
        native.push(match value {
            Value::Nil => NativeArgument::Nil,
            Value::Boolean(value) => NativeArgument::Boolean(*value),
            Value::Integer(value) => NativeArgument::Integer(*value),
            Value::Number(value) => NativeArgument::Number(*value),
            Value::String(_) => NativeArgument::Bytes(&next_string.next().unwrap()[..]),
            Value::UserData(_) => NativeArgument::Bytes(
                next_buffer
                    .next()
                    .unwrap()
                    .data()
                    .map_err(|error| mlua::Error::runtime(error.to_string()))?,
            ),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "shroudforge.native.call argument {} is a {}. Pass nil, a boolean, a number, a string or a buffer",
                    index + 2,
                    other.type_name()
                )));
            }
        });
    }
    match native_bridge::call(mod_id, name, &native) {
        Ok(results) if results.is_empty() => Ok(MultiValue::from_vec(vec![Value::Boolean(true)])),
        Ok(results) => results
            .into_iter()
            .map(|result| {
                Ok(match result {
                    NativeResult::Nil => Value::Nil,
                    NativeResult::Boolean(value) => Value::Boolean(value),
                    NativeResult::Integer(value) => Value::Integer(value),
                    NativeResult::Number(value) => Value::Number(value),
                    NativeResult::Bytes(value) => Value::String(lua.create_string(value)?),
                })
            })
            .collect(),
        Err(reason) => Ok(MultiValue::from_vec(vec![
            Value::Nil,
            Value::String(lua.create_string(reason)?),
        ])),
    }
}

#[cfg(test)]
#[path = "call_mod_native_functions_tests.rs"]
mod tests;
