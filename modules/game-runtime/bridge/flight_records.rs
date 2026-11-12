//! Optional ABI12 Flight control-plane record. No native/game pointer escapes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FlightClientStatus {
    pub struct_bytes: u32,
    pub version: u32,
    pub flags: u32,
    pub mode: u32,
    pub local_reset_serial: u64,
    pub reserved0: u32,
    pub reserved1: u32,
}
impl FlightClientStatus {
    pub fn request() -> Self {
        Self {
            struct_bytes: 32,
            version: 1,
            ..Self::default()
        }
    }
}
const _: () = assert!(std::mem::size_of::<FlightClientStatus>() == 32);
