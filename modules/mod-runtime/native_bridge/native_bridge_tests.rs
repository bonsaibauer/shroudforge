use super::*;
use std::{cell::RefCell, path::Path};

thread_local! {
    // Codes returned to the test functions. A panic must not cross extern "C".
    static CODES: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

fn record_code(code: u32) {
    CODES.with(|codes| codes.borrow_mut().push(code));
}

fn take_codes() -> Vec<u32> {
    CODES.with(|codes| std::mem::take(&mut *codes.borrow_mut()))
}

/// Returns every argument unchanged. Bytes are copied by push.
unsafe extern "C" fn echo(
    _context: *mut c_void,
    arguments: *const NativeValue,
    count: usize,
    results: *mut NativeResults,
) {
    let arguments = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(arguments, count) }
    };
    for argument in arguments {
        unsafe { ((*results).push)(results, argument) };
    }
}

/// Pushes one value, then fails. A later push must be refused.
unsafe extern "C" fn refuse(
    _context: *mut c_void,
    _arguments: *const NativeValue,
    _count: usize,
    results: *mut NativeResults,
) {
    let value = NativeValue {
        kind: KIND_INTEGER,
        reserved: 0,
        payload: NativePayload { integer: 7 },
    };
    unsafe {
        ((*results).push)(results, &value);
        ((*results).fail)(results, c"no target under the cursor".as_ptr());
        ((*results).fail)(results, c"second reason".as_ptr());
        record_code(((*results).push)(results, &value));
    }
}

/// Reports the context it was registered with.
unsafe extern "C" fn context_value(
    context: *mut c_void,
    _arguments: *const NativeValue,
    _count: usize,
    results: *mut NativeResults,
) {
    let value = NativeValue {
        kind: KIND_INTEGER,
        reserved: 0,
        payload: NativePayload {
            integer: context as i64,
        },
    };
    unsafe { ((*results).push)(results, &value) };
}

/// Pushes invalid values, then more values than allowed, recording each code.
unsafe extern "C" fn overflow(
    _context: *mut c_void,
    _arguments: *const NativeValue,
    _count: usize,
    results: *mut NativeResults,
) {
    let value = NativeValue {
        kind: KIND_NIL,
        reserved: 0,
        payload: NativePayload { integer: 0 },
    };
    let invalid = NativeValue {
        kind: 99,
        ..value
    };
    unsafe {
        record_code(((*results).push)(results, &invalid));
        record_code(((*results).push)(results, std::ptr::null()));
        for _ in 0..=MAX_RESULTS {
            record_code(((*results).push)(results, &value));
        }
    }
}

fn register(
    host: &'static registration::HostRecord,
    name: &std::ffi::CStr,
    function: NativeFunction,
    context: usize,
) -> u32 {
    let host = host.host();
    unsafe {
        ((*host).register_function)(host, name.as_ptr(), Some(function), context as *mut c_void)
    }
}

#[test]
fn host_describes_the_owning_mod_and_its_abi() {
    let record = open_host("bridge.host", Path::new("C:/Game/mods/bridge-host")).unwrap();
    let host = unsafe { &*record.host() };
    assert_eq!(host.abi_version, ABI_VERSION);
    assert_eq!(host.size as usize, size_of::<ModHost>());
    assert_eq!(
        unsafe { std::ffi::CStr::from_ptr(host.mod_id) }.to_str(),
        Ok("bridge.host")
    );
    assert_eq!(
        unsafe { std::ffi::CStr::from_ptr(host.mod_directory) }.to_str(),
        Ok("C:/Game/mods/bridge-host")
    );
    // Logging is safe from any state, including a missing message.
    unsafe {
        (host.log)(record.host(), 2, c"bridge test line".as_ptr());
        (host.log)(record.host(), 9, std::ptr::null());
        (host.log)(std::ptr::null(), 2, c"ignored".as_ptr());
    }
    assert!(open_host("bad\0id", Path::new("C:/Game")).is_err());
}

#[test]
fn values_cross_the_bridge_unchanged() {
    let record = open_host("bridge.values", Path::new("mods/values")).unwrap();
    assert_eq!(register(record, c"echo", echo, 0), 1);
    close_registration(record, true);

    let bytes = [0_u8, 1, 2, 255];
    let results = call(
        "bridge.values",
        "echo",
        &[
            NativeArgument::Nil,
            NativeArgument::Boolean(true),
            NativeArgument::Boolean(false),
            NativeArgument::Integer(-1 << 40),
            NativeArgument::Number(0.25),
            NativeArgument::Bytes(&bytes),
            NativeArgument::Bytes(&[]),
        ],
    )
    .unwrap();
    assert_eq!(
        results,
        vec![
            NativeResult::Nil,
            NativeResult::Boolean(true),
            NativeResult::Boolean(false),
            NativeResult::Integer(-1 << 40),
            NativeResult::Number(0.25),
            NativeResult::Bytes(bytes.to_vec()),
            NativeResult::Bytes(Vec::new()),
        ]
    );
    assert_eq!(call("bridge.values", "echo", &[]).unwrap(), Vec::new());
}

#[test]
fn a_failure_keeps_its_first_reason_and_drops_pushed_values() {
    let record = open_host("bridge.failure", Path::new("mods/failure")).unwrap();
    assert_eq!(register(record, c"refuse", refuse, 0), 1);
    close_registration(record, true);
    take_codes();
    assert_eq!(
        call("bridge.failure", "refuse", &[]),
        Err("no target under the cursor".into())
    );
    assert_eq!(take_codes(), vec![4]);
}

#[test]
fn results_are_bounded_and_invalid_values_are_refused() {
    let record = open_host("bridge.overflow", Path::new("mods/overflow")).unwrap();
    assert_eq!(register(record, c"overflow", overflow, 0), 1);
    close_registration(record, true);
    take_codes();
    let results = call("bridge.overflow", "overflow", &[]).unwrap();
    assert_eq!(results.len(), MAX_RESULTS);
    let mut expected = vec![2, 2];
    expected.extend(std::iter::repeat_n(1, MAX_RESULTS));
    expected.push(3);
    assert_eq!(take_codes(), expected);
}

#[test]
fn registration_checks_names_duplicates_and_its_window() {
    let record = open_host("bridge.registration", Path::new("mods/registration")).unwrap();
    let host = record.host();
    assert_eq!(register(record, c"view.build", context_value, 41), 1);
    assert_eq!(register(record, c"view.build", echo, 0), 3);
    for invalid in [c"", c"1st", c".hidden", c"with space", c"\xc3\xa4"] {
        assert_eq!(register(record, invalid, echo, 0), 2);
    }
    let long = std::ffi::CString::new("a".repeat(65)).unwrap();
    assert_eq!(register(record, &long, echo, 0), 2);
    unsafe {
        assert_eq!(
            ((*host).register_function)(host, std::ptr::null(), Some(echo), std::ptr::null_mut()),
            2
        );
        assert_eq!(
            ((*host).register_function)(host, c"missing".as_ptr(), None, std::ptr::null_mut()),
            2
        );
    }
    close_registration(record, true);
    assert_eq!(register(record, c"late", echo, 0), 4);
    assert_eq!(function_names("bridge.registration"), vec!["view.build"]);
    assert_eq!(
        call("bridge.registration", "view.build", &[]).unwrap(),
        vec![NativeResult::Integer(41)]
    );
}

#[test]
fn a_failed_initialization_withdraws_only_its_own_functions() {
    let kept = open_host("bridge.withdraw", Path::new("mods/withdraw")).unwrap();
    assert_eq!(register(kept, c"kept", echo, 0), 1);
    close_registration(kept, true);
    let failed = open_host("bridge.withdraw", Path::new("mods/withdraw")).unwrap();
    assert_eq!(register(failed, c"dropped", echo, 0), 1);
    close_registration(failed, false);
    assert_eq!(function_names("bridge.withdraw"), vec!["kept"]);
    assert!(
        call("bridge.withdraw", "dropped", &[])
            .unwrap_err()
            .contains("not registered")
    );
}

#[test]
fn a_mod_reaches_only_its_own_functions() {
    let record = open_host("bridge.owner", Path::new("mods/owner")).unwrap();
    assert_eq!(register(record, c"secret", echo, 0), 1);
    close_registration(record, true);
    assert!(function_names("bridge.other").is_empty());
    assert!(call("bridge.other", "secret", &[]).is_err());
    assert!(call("bridge.owner", "secret", &[]).is_ok());
}

#[test]
fn function_names_are_readable_identifiers() {
    for valid in ["build", "view3d.read", "_private", "A1_b.c2"] {
        assert!(valid_function_name(valid), "{valid}");
    }
    for invalid in ["", "9lives", ".a", "a b", "a-b", "a/b", "ä"] {
        assert!(!valid_function_name(invalid), "{invalid}");
    }
    assert!(valid_function_name(&"a".repeat(64)));
    assert!(!valid_function_name(&"a".repeat(65)));
}

#[test]
fn the_local_camera_follows_the_mods_runtime_access() {
    let record = open_host("bridge.camera", Path::new("mods/camera")).unwrap();
    close_registration(record, true);
    let host = record.host();
    let mut view = [0.0_f64; VIEW_VALUES];
    let read = |output: *mut f64, count: usize| unsafe {
        ((*host).read_local_camera)(host, output, count)
    };
    assert_eq!(read(std::ptr::null_mut(), VIEW_VALUES), 2);
    assert_eq!(read(view.as_mut_ptr(), CAMERA_VALUES - 1), 2);
    assert_eq!(unsafe { ((*host).read_local_camera)(std::ptr::null(), view.as_mut_ptr(), 9) }, 2);
    // Not an active runtime mod: no game read, whatever the runtime offers.
    assert_eq!(read(view.as_mut_ptr(), VIEW_VALUES), 7);
    set_game_access("bridge.camera", true);
    assert!(has_game_access("bridge.camera"));
    assert!(!has_game_access("bridge.other"));
    // No game runtime in unit tests: the reader is reported missing.
    assert_eq!(read(view.as_mut_ptr(), VIEW_VALUES), 8);
    set_game_access("bridge.camera", false);
    assert_eq!(read(view.as_mut_ptr(), VIEW_VALUES), 7);
}
