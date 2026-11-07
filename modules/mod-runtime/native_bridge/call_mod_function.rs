use super::*;

/// A Lua argument as the native function sees it. Bytes borrow Lua memory.
pub(crate) enum NativeArgument<'a> {
    Nil,
    Boolean(bool),
    Integer(i64),
    Number(f64),
    Bytes(&'a [u8]),
}

/// A value returned by the native function, copied into ShroudForge memory.
#[derive(Debug, PartialEq)]
pub(crate) enum NativeResult {
    Nil,
    Boolean(bool),
    Integer(i64),
    Number(f64),
    Bytes(Vec<u8>),
}

#[derive(Default)]
struct Collected {
    values: Vec<NativeResult>,
    failure: Option<String>,
}

/// Calls one of `mod_id`'s registered functions on the calling thread.
/// `Err` carries the native function's reason, or why it could not be called.
pub(crate) fn call(
    mod_id: &str,
    name: &str,
    arguments: &[NativeArgument],
) -> Result<Vec<NativeResult>, String> {
    let Some((function, context)) = registration::find(mod_id, name) else {
        return Err(format!(
            "native function '{name}' is not registered by this mod's DLLs"
        ));
    };
    let values: Vec<NativeValue> = arguments.iter().map(native_value).collect();
    let mut collected = Collected::default();
    let mut results = NativeResults {
        host_state: (&raw mut collected).cast(),
        push: push_result,
        fail: fail_call,
    };
    let pointer = if values.is_empty() {
        std::ptr::null()
    } else {
        values.as_ptr()
    };
    unsafe { function(context as *mut c_void, pointer, values.len(), &mut results) };
    match collected.failure {
        Some(reason) => Err(reason),
        None => Ok(collected.values),
    }
}

fn native_value(argument: &NativeArgument) -> NativeValue {
    // Booleans and nil are written as 64 bits so the whole payload is defined.
    let (kind, payload) = match *argument {
        NativeArgument::Nil => (KIND_NIL, NativePayload { integer: 0 }),
        NativeArgument::Boolean(value) => (
            KIND_BOOLEAN,
            NativePayload {
                integer: i64::from(value),
            },
        ),
        NativeArgument::Integer(value) => (KIND_INTEGER, NativePayload { integer: value }),
        NativeArgument::Number(value) => (KIND_NUMBER, NativePayload { number: value }),
        NativeArgument::Bytes(value) => (
            KIND_BYTES,
            NativePayload {
                bytes: NativeBytes {
                    data: value.as_ptr().cast(),
                    size: value.len(),
                },
            },
        ),
    };
    NativeValue {
        kind,
        reserved: 0,
        payload,
    }
}

/// `results` must be null or the pointer handed to the native function.
unsafe fn collected<'a>(results: *mut NativeResults) -> Option<&'a mut Collected> {
    // host_state always points at the Collected of the call in progress.
    let results = unsafe { results.as_ref() }?;
    unsafe { results.host_state.cast::<Collected>().as_mut() }
}

unsafe extern "C" fn push_result(results: *mut NativeResults, value: *const NativeValue) -> u32 {
    let Some(collected) = (unsafe { collected(results) }) else {
        return 2;
    };
    if collected.failure.is_some() {
        return 4;
    }
    let Some(value) = (unsafe { value.as_ref() }) else {
        return 2;
    };
    let result = match value.kind {
        KIND_NIL => NativeResult::Nil,
        KIND_BOOLEAN => NativeResult::Boolean(unsafe { value.payload.boolean } != 0),
        KIND_INTEGER => NativeResult::Integer(unsafe { value.payload.integer }),
        KIND_NUMBER => NativeResult::Number(unsafe { value.payload.number }),
        KIND_BYTES => {
            let bytes = unsafe { value.payload.bytes };
            if bytes.size == 0 {
                NativeResult::Bytes(Vec::new())
            } else if bytes.data.is_null() {
                return 2;
            } else {
                NativeResult::Bytes(
                    unsafe { std::slice::from_raw_parts(bytes.data.cast::<u8>(), bytes.size) }
                        .to_vec(),
                )
            }
        }
        _ => return 2,
    };
    if collected.values.len() >= MAX_RESULTS {
        return 3;
    }
    collected.values.push(result);
    1
}

unsafe extern "C" fn fail_call(results: *mut NativeResults, reason: *const c_char) {
    let Some(collected) = (unsafe { collected(results) }) else {
        return;
    };
    if collected.failure.is_none() {
        collected.failure = Some(if reason.is_null() {
            "native function failed without a reason".into()
        } else {
            unsafe { std::ffi::CStr::from_ptr(reason) }
                .to_string_lossy()
                .into_owned()
        });
    }
    collected.values.clear();
}
