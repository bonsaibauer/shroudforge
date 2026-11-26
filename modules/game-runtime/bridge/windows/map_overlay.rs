//! Control-plane forwarding only. Frame callbacks never enter Rust or Lua.
use super::native_abi::{
    DrainMapOverlayServices, QueryMapOverlayService, RevokeMapOverlayServices,
};
use super::provider::provider;
use std::ffi::c_void;

/// A table can escape to a plugin only if its entire retirement surface exists.
#[derive(Clone, Copy)]
pub(super) struct MapOverlayExports {
    query: QueryMapOverlayService,
    revoke: RevokeMapOverlayServices,
    drain: DrainMapOverlayServices,
}

impl MapOverlayExports {
    pub(super) fn new(
        query: Option<QueryMapOverlayService>,
        revoke: Option<RevokeMapOverlayServices>,
        drain: Option<DrainMapOverlayServices>,
    ) -> Option<Self> {
        Some(Self {
            query: query?,
            revoke: revoke?,
            drain: drain?,
        })
    }
}
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
    unsafe {
        forward_query(
            provider().and_then(|p| p.map_overlay),
            package,
            generation,
            name,
            version,
            capacity,
            output,
        )
    }
}

unsafe fn forward_query(
    exports: Option<MapOverlayExports>,
    package: &str,
    generation: u64,
    name: &str,
    version: u32,
    capacity: u32,
    output: *mut c_void,
) -> i32 {
    let Some(exports) = exports else {
        return 3;
    };
    unsafe {
        (exports.query)(
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
    let Some(exports) = provider().and_then(|p| p.map_overlay) else {
        // No table can have been exposed by an absent or incomplete capability.
        return 0;
    };
    unsafe { (exports.revoke)(package.as_ptr(), package.len() as u32, generation) }
}
pub fn drain_map_overlay_services(package: &str, generation: u64, timeout: u32) -> i32 {
    if package.is_empty() || package.len() > 256 || generation == 0 || timeout > 250 {
        return 1;
    }
    let Some(exports) = provider().and_then(|p| p.map_overlay) else {
        return 0;
    };
    unsafe { (exports.drain)(package.as_ptr(), package.len() as u32, generation, timeout) }
}

pub fn map_overlay_status() -> Result<serde_json::Value, String> {
    let call = provider()
        .and_then(|p| p.map_overlay_status_json)
        .ok_or("map/overlay status export unavailable")?;
    super::provider::fetch_json(
        |output, capacity| unsafe { call(output, capacity) },
        64 * 1024,
        super::provider::CATALOG_ERRORS,
    )
}
pub fn map_overlay_operation_available(map: bool) -> bool {
    let Some(p) = provider() else { return false };
    if p.map_overlay.is_none()
        || (map && p.map_snapshot_json.is_none())
        || (!map && p.submit_overlay_scene_json.is_none())
    {
        return false;
    }
    map_overlay_status().is_ok_and(|status| {
        status[if map {
            "map_prepared"
        } else {
            "overlay_prepared"
        }] == true
    })
}
pub fn map_snapshot(
    package: &str,
    generation: u64,
    expected_session: u64,
    view: u32,
) -> Result<serde_json::Value, String> {
    if package.is_empty() || package.len() > 256 || generation == 0 {
        return Err("invalid map package scope".into());
    }
    let p = provider().ok_or("map provider unavailable")?;
    if p.map_overlay.is_none() {
        return Err("map service retirement exports unavailable".into());
    }
    let call = p
        .map_snapshot_json
        .ok_or("map snapshot export unavailable")?;
    let value = super::provider::fetch_json(
        |output, capacity| unsafe {
            call(
                package.as_ptr(),
                package.len() as u32,
                generation,
                expected_session,
                view,
                output,
                capacity,
            )
        },
        64 * 1024 * 1024,
        super::provider::CATALOG_ERRORS,
    )?;
    if value["service_status"] != 0 {
        return Err(format!(
            "map service unavailable, status {}",
            value["service_status"]
        ));
    }
    value
        .get("frame")
        .cloned()
        .ok_or("map snapshot response has no frame".into())
}
pub fn submit_overlay_scene(package: &str, generation: u64, json: &[u8]) -> Result<(), String> {
    if package.is_empty()
        || package.len() > 256
        || generation == 0
        || json.is_empty()
        || json.len() > 8 * 1024 * 1024
    {
        return Err("invalid overlay scene input or package scope".into());
    }
    let p = provider().ok_or("overlay provider unavailable")?;
    if p.map_overlay.is_none() {
        return Err("overlay service retirement exports unavailable".into());
    }
    let call = p
        .submit_overlay_scene_json
        .ok_or("overlay scene export unavailable")?;
    let status = unsafe {
        call(
            package.as_ptr(),
            package.len() as u32,
            generation,
            json.as_ptr(),
            json.len() as u32,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("overlay submission failed, status {status}"))
    }
}

#[cfg(test)]
mod map_overlay_tests {
    use super::*;
    #[test]
    fn partial_providers_cannot_publish_service_tables() {
        unsafe extern "C" fn query(
            _: *const u8,
            _: u32,
            _: u64,
            _: *const u8,
            _: u32,
            _: u32,
            _: u32,
            output: *mut c_void,
        ) -> i32 {
            unsafe {
                *output.cast::<u32>() = 77;
            }
            0
        }
        unsafe extern "C" fn revoke(_: *const u8, _: u32, _: u64) -> i32 {
            6
        }
        unsafe extern "C" fn drain(_: *const u8, _: u32, _: u64, _: u32) -> i32 {
            7
        }
        for mask in 0..8 {
            let exports = MapOverlayExports::new(
                (mask & 1 != 0).then_some(query as QueryMapOverlayService),
                (mask & 2 != 0).then_some(revoke as RevokeMapOverlayServices),
                (mask & 4 != 0).then_some(drain as DrainMapOverlayServices),
            );
            let mut output = 0u32;
            let result = unsafe {
                forward_query(
                    exports,
                    "test",
                    1,
                    "shroudforge.map",
                    1,
                    4,
                    (&mut output as *mut u32).cast(),
                )
            };
            if mask == 7 {
                assert_eq!((result, output), (0, 77));
                let exports = exports.unwrap();
                assert_eq!(unsafe { (exports.revoke)(b"test".as_ptr(), 4, 1) }, 6);
                assert_eq!(unsafe { (exports.drain)(b"test".as_ptr(), 4, 1, 250) }, 7);
            } else {
                assert_eq!((result, output), (3, 0), "partial export mask {mask}");
            }
        }
    }
    #[test]
    fn rejects_unbounded_control_inputs_before_provider_access() {
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
