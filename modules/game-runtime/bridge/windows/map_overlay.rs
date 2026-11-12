//! Control-plane forwarding only. Frame callbacks never enter Rust or Lua.
use super::provider::provider;
use std::ffi::c_void;
pub fn native_publish_notification(id: &str, title: &str, message: &str, level: u32) -> i32 {
    if id.is_empty()
        || id.len() > 256
        || title.is_empty()
        || title.len() > 120
        || message.is_empty()
        || message.len() > 512
        || level > 4
    {
        return 1;
    }
    let Some(call) = provider().and_then(|p| p.publish_notification) else {
        return 3;
    };
    unsafe {
        call(
            id.as_ptr(),
            id.len() as u32,
            title.as_ptr(),
            title.len() as u32,
            message.as_ptr(),
            message.len() as u32,
            level,
        )
    }
}
/// # Safety
/// `output` must identify a writable buffer of `capacity` bytes. The caller
/// establishes package identity, capability and process target before calling.
pub unsafe fn query_map_overlay_service(
    package: &str,
    generation: u64,
    name: &str,
    version: u32,
    capacity: u32,
    output: *mut c_void,
) -> i32 {
    if package.is_empty()
        || package.len() > 256
        || name.is_empty()
        || name.len() > 64
        || generation == 0
        || output.is_null()
    {
        return 1;
    }
    let Some(call) = provider().and_then(|p| p.query_map_overlay_service) else {
        return 3;
    };
    unsafe {
        call(
            package.as_ptr(),
            package.len() as u32,
            generation,
            name.as_ptr(),
            name.len() as u32,
            version,
            capacity,
            output,
        )
    }
}
pub fn revoke_map_overlay_services(package: &str, generation: u64) -> i32 {
    if package.is_empty() || package.len() > 256 || generation == 0 {
        return 1;
    }
    let Some(call) = provider().and_then(|p| p.revoke_map_overlay_services) else {
        return 0;
    };
    unsafe { call(package.as_ptr(), package.len() as u32, generation) }
}
pub fn drain_map_overlay_services(package: &str, generation: u64, timeout: u32) -> i32 {
    if package.is_empty() || package.len() > 256 || generation == 0 || timeout > 250 {
        return 1;
    }
    let Some(call) = provider().and_then(|p| p.drain_map_overlay_services) else {
        return 0;
    };
    unsafe { call(package.as_ptr(), package.len() as u32, generation, timeout) }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct InputKeyState {
    pub virtual_key: u32,
    pub flags: u32,
    pub sequence: u64,
    pub sampled_at_ms: u64,
}
pub fn read_input_keys(keys: &[u32]) -> Result<Vec<InputKeyState>, i32> {
    if keys.is_empty() || keys.len() > 32 || keys.iter().any(|key| *key == 0 || *key > 255) {
        return Err(1);
    }
    let Some(call) = provider().and_then(|p| p.read_input_keys) else {
        return Err(3);
    };
    let mut output = vec![InputKeyState::default(); keys.len()];
    let status = unsafe {
        call(
            keys.as_ptr(),
            keys.len() as u32,
            std::mem::size_of_val(output.as_slice()) as u32,
            output.as_mut_ptr().cast(),
        )
    };
    if status == 0 { Ok(output) } else { Err(status) }
}
#[cfg(test)]
mod map_overlay_tests {
    use super::*;
    #[test]
    fn rejects_unbounded_control_inputs_before_provider_access() {
        assert_eq!(read_input_keys(&[]).unwrap_err(), 1);
        assert_eq!(read_input_keys(&[0]).unwrap_err(), 1);
        assert_eq!(read_input_keys(&[256]).unwrap_err(), 1);
        assert_eq!(read_input_keys(&[1; 33]).unwrap_err(), 1);
        assert_eq!(std::mem::size_of::<InputKeyState>(), 24);
        assert_eq!(
            unsafe {
                query_map_overlay_service(
                    "",
                    1,
                    "shroudforge.map",
                    1,
                    4096,
                    std::ptr::dangling_mut(),
                )
            },
            1
        );
        assert_eq!(drain_map_overlay_services("valid", 1, 251), 1);
    }
}
