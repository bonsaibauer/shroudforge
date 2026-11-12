//! Rust binding of the canonical native runtime ABI.
const KFC_RUNTIME_ABI_VERSION: u32 = 12;

mod flight_records;
mod runtime_records;
pub use flight_records::FlightClientStatus;
mod vein_notice_records;
pub use vein_notice_records::{VeinNativeNotices, VeinNoticeBatch};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
#[cfg(not(windows))]
mod unsupported_platform;
#[cfg(not(windows))]
pub use unsupported_platform::*;

#[path = "building_input.rs"]
pub mod player_input;

pub const fn first_person_operation_name() -> &'static str {
    "runtime.first_person.set_mode"
}

mod map_views;
pub use map_views::{map_fog, map_markers, map_overlay_status, map_summary};
mod notifications;
pub use notifications::publish_notification;
