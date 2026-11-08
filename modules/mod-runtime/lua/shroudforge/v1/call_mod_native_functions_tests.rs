use super::*;
use crate::native_bridge::{
    KIND_BYTES, KIND_INTEGER, NativePayload, NativeResults, NativeValue, close_registration,
    open_host,
};
use std::{ffi::c_void, path::Path};

/// Returns every argument unchanged.
unsafe extern "C" fn echo(
    _context: *mut c_void,
    arguments: *const NativeValue,
    count: usize,
    results: *mut NativeResults,
) {
    for index in 0..count {
        unsafe { ((*results).push)(results, arguments.add(index)) };
    }
}

/// Returns where the first argument's bytes live and how many there are.
unsafe extern "C" fn locate(
    _context: *mut c_void,
    arguments: *const NativeValue,
    count: usize,
    results: *mut NativeResults,
) {
    if count == 0 || unsafe { (*arguments).kind } != KIND_BYTES {
        unsafe { ((*results).fail)(results, c"expected bytes".as_ptr()) };
        return;
    }
    let bytes = unsafe { (*arguments).payload.bytes };
    for integer in [bytes.data as i64, bytes.size as i64] {
        let value = NativeValue {
            kind: KIND_INTEGER,
            reserved: 0,
            payload: NativePayload { integer },
        };
        unsafe { ((*results).push)(results, &value) };
    }
}

/// Succeeds without returning a value.
unsafe extern "C" fn nothing(
    _context: *mut c_void,
    _arguments: *const NativeValue,
    _count: usize,
    _results: *mut NativeResults,
) {
}

fn lua_for(mod_id: &str) -> mlua::Result<Lua> {
    let record = open_host(mod_id, Path::new("mods/lua-test")).unwrap();
    let host = record.host();
    for (name, function) in [
        (c"echo", echo as crate::native_bridge::NativeFunction),
        (c"locate", locate),
        (c"nothing", nothing),
    ] {
        let code = unsafe {
            ((*host).register_function)(host, name.as_ptr(), Some(function), std::ptr::null_mut())
        };
        assert_eq!(code, 1);
    }
    close_registration(record, true);
    let lua = Lua::new();
    lua.globals().set("native", create(&lua, mod_id)?)?;
    Ok(lua)
}

#[test]
fn lua_values_round_trip_through_a_native_function() -> mlua::Result<()> {
    let lua = lua_for("bridge.lua.values")?;
    lua.load(
        r##"
        local a, b, c, d, e, f = native.call("echo", nil, true, 42, 0.5, "Block äß", "")
        assert(a == nil and b == true and c == 42 and math.type(c) == "integer")
        assert(d == 0.5 and e == "Block äß" and f == "")
        assert(select("#", native.call("echo", nil)) == 1)
        assert(native.call("nothing") == true)
        local names = native.functions()
        assert(#names == 3 and names[1] == "echo" and names[2] == "locate" and names[3] == "nothing")
        "##,
    )
    .exec()
}

#[test]
fn buffers_are_passed_without_a_copy() -> mlua::Result<()> {
    let lua = lua_for("bridge.lua.buffer")?;
    let buffer = lua.create_userdata(Buffer::wrap(vec![1, 2, 3, 4, 5]))?;
    let address = buffer.borrow::<Buffer>()?.data().unwrap().as_ptr() as i64;
    lua.globals().set("buffer", buffer)?;
    let (located, size): (i64, i64) = lua.load(r##"return native.call("locate", buffer)"##).eval()?;
    assert_eq!((located, size), (address, 5));
    let echoed: mlua::String = lua.load(r##"return native.call("echo", buffer)"##).eval()?;
    assert_eq!(&echoed.as_bytes()[..], &[1, 2, 3, 4, 5]);
    Ok(())
}

#[test]
fn failures_return_nil_and_a_reason() -> mlua::Result<()> {
    let lua = lua_for("bridge.lua.failure")?;
    lua.load(
        r##"
        local value, reason = native.call("locate", 3)
        assert(value == nil and reason == "expected bytes")
        value, reason = native.call("missing")
        assert(value == nil and reason:find("not registered", 1, true))
        "##,
    )
    .exec()
}

#[test]
fn unsupported_arguments_raise_a_lua_error() -> mlua::Result<()> {
    let lua = lua_for("bridge.lua.arguments")?;
    for argument in ["{}", "function() end", "coroutine.create(function() end)"] {
        let error = lua
            .load(format!(r##"return native.call("echo", 1, {argument})"##))
            .exec()
            .unwrap_err()
            .to_string();
        assert!(error.contains("argument 3"), "{error}");
    }
    Ok(())
}

#[test]
fn each_mod_sees_only_its_own_functions() -> mlua::Result<()> {
    lua_for("bridge.lua.owner")?;
    let lua = Lua::new();
    lua.globals().set("native", create(&lua, "bridge.lua.stranger")?)?;
    lua.load(
        r##"
        assert(#native.functions() == 0)
        local value, reason = native.call("echo", 1)
        assert(value == nil and reason ~= nil)
        "##,
    )
    .exec()
}
