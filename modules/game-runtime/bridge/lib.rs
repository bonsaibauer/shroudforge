//! Rust binding of the canonical native runtime ABI.
const KFC_RUNTIME_ABI_VERSION: u32 = 12;

mod runtime_records;

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

mod notifications;
pub use notifications::publish_notification;
