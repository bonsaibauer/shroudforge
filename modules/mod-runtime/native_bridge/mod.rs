//! Bridge between a mod's own native DLL and the same mod's Lua code.
//! The C contract is `shroudforge_mod_native.h`. These types mirror it.
//! Registrations are process-owned like the DLLs: Lua reloads find them again.

use std::ffi::{c_char, c_void};

#[path = "call_mod_function.rs"]
mod call;
mod read_local_camera;
#[path = "register_mod_functions.rs"]
mod registration;
#[path = "track_game_access.rs"]
mod access;

pub(crate) use access::{has_game_access, set_game_access};
pub(crate) use call::{NativeArgument, NativeResult, call};
pub(crate) use registration::{function_names, initialize};
#[cfg(test)]
pub(crate) use registration::{close_registration, open_host};

pub(crate) const ABI_VERSION: u32 = 2;
pub(crate) const MAX_RESULTS: usize = 32;
/// Camera: position x, y, z in meters, orientation x, y, z, w, fovY, aspect.
pub(crate) const CAMERA_VALUES: usize = 9;
/// Camera, then cursor: position x, y, z in meters, orientation, flags.
pub(crate) const VIEW_VALUES: usize = 17;
pub(crate) const INIT_EXPORT: &std::ffi::CStr = c"ShroudforgeModNativeInit";

pub(crate) const KIND_NIL: u32 = 0;
pub(crate) const KIND_BOOLEAN: u32 = 1;
pub(crate) const KIND_INTEGER: u32 = 2;
pub(crate) const KIND_NUMBER: u32 = 3;
pub(crate) const KIND_BYTES: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NativeBytes {
    pub data: *const c_void,
    pub size: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) union NativePayload {
    pub boolean: i32,
    pub integer: i64,
    pub number: f64,
    pub bytes: NativeBytes,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NativeValue {
    pub kind: u32,
    pub reserved: u32,
    pub payload: NativePayload,
}

#[repr(C)]
pub(crate) struct NativeResults {
    pub host_state: *mut c_void,
    pub push: unsafe extern "C" fn(*mut NativeResults, *const NativeValue) -> u32,
    pub fail: unsafe extern "C" fn(*mut NativeResults, *const c_char),
}

pub(crate) type NativeFunction =
    unsafe extern "C" fn(*mut c_void, *const NativeValue, usize, *mut NativeResults);

#[repr(C)]
pub(crate) struct ModHost {
    pub abi_version: u32,
    pub size: u32,
    pub mod_id: *const c_char,
    pub mod_directory: *const c_char,
    pub log: unsafe extern "C" fn(*const ModHost, u32, *const c_char),
    pub register_function:
        unsafe extern "C" fn(*const ModHost, *const c_char, Option<NativeFunction>, *mut c_void) -> u32,
    // ABI 2.
    pub read_local_camera: unsafe extern "C" fn(*const ModHost, *mut f64, usize) -> u32,
}

pub(crate) type InitFunction = unsafe extern "C" fn(*const ModHost) -> i32;

/// Function names stay readable in logs and Lua: ASCII identifiers with dots.
pub(crate) fn valid_function_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

#[cfg(test)]
#[path = "native_bridge_tests.rs"]
mod tests;
