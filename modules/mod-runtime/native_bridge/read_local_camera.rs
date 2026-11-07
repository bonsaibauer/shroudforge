use super::*;

/// Host function of ABI 2: the local camera, and the building cursor when
/// `count` allows, for a DLL that draws every frame. Any thread, read only.
pub(super) unsafe extern "C" fn host_read_local_camera(
    host: *const ModHost,
    output: *mut f64,
    count: usize,
) -> u32 {
    let Some(record) = (unsafe { registration::record(host) }) else {
        return 2;
    };
    if output.is_null() || count < CAMERA_VALUES {
        return 2;
    }
    if !has_game_access(record.mod_id()) {
        return 7;
    }
    let output = unsafe { std::slice::from_raw_parts_mut(output, count.min(VIEW_VALUES)) };
    shroudforge_runtime_bridge::read_local_camera(output).unwrap_or(8)
}
