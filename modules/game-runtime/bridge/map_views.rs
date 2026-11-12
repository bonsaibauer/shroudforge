//! Bounded control-plane map reads. Native frame traffic stays inside the host.
use serde_json::{Value, json};
use std::{ffi::c_void, mem::size_of};
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Meta {
    status: u32,
    freshness: u32,
    generation: u64,
    revision: u64,
    at: u64,
    clock: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Pose {
    position: [f32; 3],
    heading: f32,
    flags: u32,
    reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Player {
    meta: Meta,
    pose: Pose,
    camera: Pose,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Marker {
    id: u64,
    position: [f32; 3],
    heading: f32,
    kind: u32,
    flags: u32,
    expires: u64,
    candidate_count: u32,
    reserved: u32,
    candidates: [u32; 10],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Markers {
    meta: Meta,
    data: *const Marker,
    count: u32,
    reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Fog {
    meta: Meta,
    world_width: f32,
    world_height: f32,
    columns: u32,
    rows: u32,
    tiles: *const u8,
    bytes: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Environment {
    clock: Meta,
    weather: Meta,
    minute: i32,
    daytime: u32,
    weather_kind: u32,
    reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Frame {
    size: u32,
    version: u32,
    status: u32,
    flags: u32,
    generation: u64,
    clock: u64,
    now: u64,
    features: u32,
    reserved: u32,
    player: Player,
    markers: [Markers; 6],
    fog: Fog,
    environment: Environment,
}
type Acquire = unsafe extern "C" fn(*mut c_void, u64, u32, *mut Frame, *mut u64) -> i32;
type Release = unsafe extern "C" fn(*mut c_void, u64) -> i32;
#[repr(C)]
struct Service {
    size: u32,
    version: u32,
    context: *mut c_void,
    acquire: Option<Acquire>,
    release: Option<Release>,
    keys: *const c_void,
}
fn meta(value: Meta) -> Value {
    json!({"status":value.status,"session_generation":value.generation.to_string(),"revision":value.revision.to_string(),"published_at_ms":value.at.to_string(),"clock_domain":value.clock.to_string(),"freshness_limit_ms":value.freshness})
}
fn pose(value: Pose) -> Value {
    json!({"position":value.position,"heading_radians":value.heading,"flags":value.flags})
}
fn with_frame<T>(
    package: &str,
    generation: u64,
    expected: u64,
    read: impl FnOnce(&Frame) -> Result<T, i32>,
) -> Result<T, i32> {
    // These C records have only numeric, nullable pointer, and Option<fn> fields.
    let mut service: Service = unsafe { std::mem::zeroed() };
    let status = unsafe {
        crate::query_map_overlay_service(
            package,
            generation,
            "shroudforge.map",
            1,
            size_of::<Service>() as u32,
            (&mut service as *mut Service).cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    if service.version != 1 || service.size != size_of::<Service>() as u32 {
        return Err(2);
    }
    let (Some(acquire), Some(release)) = (service.acquire, service.release) else {
        return Err(2);
    };
    let mut frame: Frame = unsafe { std::mem::zeroed() };
    let mut token = 0;
    let status = unsafe {
        acquire(
            service.context,
            expected,
            size_of::<Frame>() as u32,
            &mut frame,
            &mut token,
        )
    };
    if status != 0 {
        return Err(status);
    }
    struct Lease {
        context: *mut c_void,
        token: u64,
        release: Release,
    }
    impl Drop for Lease {
        fn drop(&mut self) {
            unsafe { (self.release)(self.context, self.token) };
        }
    }
    let _lease = Lease {
        context: service.context,
        token,
        release,
    };
    if frame.version != 1 || frame.size != size_of::<Frame>() as u32 {
        return Err(2);
    }
    read(&frame)
}
#[repr(C)]
struct OverlayStatus {
    size: u32,
    version: u32,
    status: u32,
    reason: u32,
    device: u64,
    rendered: u64,
    busy: u64,
    failed: u64,
}
#[repr(C)]
struct OverlayService {
    size: u32,
    version: u32,
    context: *mut c_void,
    register: *const c_void,
    unregister: *const c_void,
    read: Option<unsafe extern "C" fn(*mut c_void, u32, *mut OverlayStatus) -> i32>,
}
/// Explicit status activation uses this Lua owner's ordinary native overlay
/// service lease. It never registers a Lua renderer or forwards frame traffic.
pub fn map_overlay_status(package: &str, generation: u64) -> Result<Value, i32> {
    let mut service: OverlayService = unsafe { std::mem::zeroed() };
    let status = unsafe {
        crate::query_map_overlay_service(
            package,
            generation,
            "shroudforge.overlay",
            1,
            size_of::<OverlayService>() as u32,
            (&mut service as *mut OverlayService).cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    if service.size != size_of::<OverlayService>() as u32 || service.version != 1 {
        return Err(2);
    }
    let Some(read) = service.read else {
        return Err(2);
    };
    let mut value: OverlayStatus = unsafe { std::mem::zeroed() };
    let status = unsafe {
        read(
            service.context,
            size_of::<OverlayStatus>() as u32,
            &mut value,
        )
    };
    if status != 0 {
        return Err(status);
    }
    Ok(
        json!({"status_domain":"SF_NATIVE_STATUS","status":value.status,"reason":value.reason,
        "action":match value.reason {1=>"requires observed swapchain recreation after interception; initial swapchain metadata is unavailable",2=>"requires a valid recreated device/swapchain",_=>""},
        "device_generation":value.device.to_string(),"rendered_frames":value.rendered.to_string(),"busy_frames":value.busy.to_string(),"failed_frames":value.failed.to_string()}),
    )
}
/// Summary contains no grid or marker arrays. IDs/timestamps are lossless strings.
pub fn map_summary(package: &str, generation: u64) -> Result<Value, i32> {
    with_frame(package, generation, 0, |f| {
        Ok(
            json!({"status":f.status,"state_flags":f.flags,"session_generation":f.generation.to_string(),"clock_domain":f.clock.to_string(),"now_ms":f.now.to_string(),"supported_features":f.features,
        "player":{"meta":meta(f.player.meta),"pose":pose(f.player.pose),"camera":pose(f.player.camera)},
        "world_markers":meta(f.markers[0].meta),"remote_players":meta(f.markers[1].meta),"deaths":meta(f.markers[2].meta),"waypoints":meta(f.markers[3].meta),"pings":meta(f.markers[4].meta),"discovery":meta(f.markers[5].meta),
        "fog":{"meta":meta(f.fog.meta),"columns":f.fog.columns,"rows":f.fog.rows,"world_width":f.fog.world_width,"world_height":f.fog.world_height,"byte_count":f.fog.bytes},
        "environment":{"clock_meta":meta(f.environment.clock),"weather_meta":meta(f.environment.weather),"minute_of_day":f.environment.minute,"daytime":f.environment.daytime!=0,"weather_kind":f.environment.weather_kind}}),
        )
    })
}
pub fn map_markers(
    package: &str,
    generation: u64,
    collection: u32,
    offset: u32,
    limit: u32,
    expected: u64,
) -> Result<Value, i32> {
    if collection >= 6 || offset > 4096 || limit == 0 || limit > 256 {
        return Err(1);
    }
    with_frame(package, generation, expected, |frame| {
        let view = frame.markers[collection as usize];
        if view.count > 4096 || (view.meta.status == 0 && view.count > 0 && view.data.is_null()) {
            return Err(7);
        }
        let mut result = Vec::new();
        if view.meta.status == 0 {
            let end = offset.saturating_add(limit).min(view.count);
            for index in offset.min(end)..end {
                let marker = unsafe { *view.data.add(index as usize) };
                if marker.candidate_count > 10 {
                    return Err(7);
                }
                result.push(json!({"id":marker.id.to_string(),"position":marker.position,"heading_radians":marker.heading,"kind":marker.kind,"flags":marker.flags,"expires_at_ms":marker.expires.to_string(),"icon_candidates":&marker.candidates[..marker.candidate_count as usize]}));
            }
        }
        Ok(json!({"meta":meta(view.meta),"total":view.count,"offset":offset,"markers":result}))
    })
}
/// At most four native 32x32 tiles per call. `revision` binds successive reads
/// to one immutable publication; callers restart if the publication changes.
pub fn map_fog(
    package: &str,
    generation: u64,
    offset: u64,
    count: u32,
    expected: u64,
    revision: u64,
) -> Result<(Value, Vec<u8>), i32> {
    if count == 0 || count > 4096 || offset > 16 * 1024 * 1024 {
        return Err(1);
    }
    with_frame(package, generation, expected, |frame| {
        let fog = frame.fog;
        if fog.meta.status != 0 {
            return Ok((meta(fog.meta), Vec::new()));
        }
        if revision != 0 && revision != fog.meta.revision {
            return Err(6);
        }
        if fog.columns == 0
            || fog.rows == 0
            || fog.columns > 128
            || fog.rows > 128
            || fog.bytes != u64::from(fog.columns) * u64::from(fog.rows) * 1024
            || fog.tiles.is_null()
        {
            return Err(7);
        }
        let end = offset.checked_add(u64::from(count)).ok_or(1)?;
        if end > fog.bytes {
            return Err(1);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(fog.tiles.add(offset as usize), count as usize) }
                .to_vec();
        Ok((meta(fog.meta), bytes))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_layout_and_read_limits() {
        assert_eq!(size_of::<Meta>(), 40);
        assert_eq!(size_of::<Marker>(), 88);
        assert_eq!(size_of::<Frame>(), 640);
        assert_eq!(size_of::<Service>(), 40);
        assert_eq!(size_of::<OverlayService>(), 40);
        assert_eq!(size_of::<OverlayStatus>(), 48);
        assert_eq!(map_markers("unused", 1, 6, 0, 1, 0).unwrap_err(), 1);
        assert_eq!(map_markers("unused", 1, 0, 0, 257, 0).unwrap_err(), 1);
        assert_eq!(map_fog("unused", 1, 0, 4097, 0, 0).unwrap_err(), 1);
    }
}
