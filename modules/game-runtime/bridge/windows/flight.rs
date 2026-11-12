//! Trusted-owner control plane only. Missing optional exports refuse activation.
use super::provider::provider;
use crate::FlightClientStatus;
#[derive(Clone, Copy)]
struct FlightFunctions {
    set: Option<unsafe extern "C" fn(u64, u32) -> i32>,
    reset: Option<unsafe extern "C" fn(u64) -> i32>,
    retire: Option<unsafe extern "C" fn(u64) -> i32>,
    status: Option<unsafe extern "C" fn(u64, *mut FlightClientStatus) -> i32>,
}
impl FlightFunctions {
    fn complete(&self) -> bool {
        self.set.is_some() && self.reset.is_some() && self.retire.is_some() && self.status.is_some()
    }
}
fn functions() -> Option<FlightFunctions> {
    let p = provider()?;
    Some(FlightFunctions {
        set: p.flight_set,
        reset: p.flight_reset,
        retire: p.flight_retire,
        status: p.flight_status,
    })
}
pub fn flight_available() -> bool {
    functions().is_some_and(|f| f.complete())
}
pub fn flight_set_enabled(token: u64, enabled: bool) -> i32 {
    invoke_set(functions(), token, enabled)
}
// This is the actual invocation boundary used above, also exercised with a
// bounded fake function table without loading any native DLL or game process.
fn invoke_set(table: Option<FlightFunctions>, token: u64, enabled: bool) -> i32 {
    if token == 0 {
        return -1;
    }
    // Never claim an owner on a partial optional ABI lacking retirement.
    let Some(table) = table.filter(FlightFunctions::complete) else {
        return 0;
    };
    let Some(call) = table.set else {
        return 0;
    };
    unsafe { call(token, u32::from(enabled)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    thread_local! {
        static MUTATIONS: Cell<u32> = const { Cell::new(0) };
        static OWNER: Cell<u64> = const { Cell::new(0) };
        static ON: Cell<u32> = const { Cell::new(0) };
    }
    unsafe extern "C" fn set(owner: u64, on: u32) -> i32 {
        MUTATIONS.set(MUTATIONS.get() + 1);
        OWNER.set(owner);
        ON.set(on);
        1
    }
    unsafe extern "C" fn reset(_: u64) -> i32 {
        1
    }
    unsafe extern "C" fn status(_: u64, _: *mut FlightClientStatus) -> i32 {
        1
    }
    fn complete() -> FlightFunctions {
        FlightFunctions {
            set: Some(set),
            reset: Some(reset),
            retire: Some(reset),
            status: Some(status),
        }
    }
    #[test]
    fn partial_flight_exports_refuse_before_native_mutation() {
        MUTATIONS.set(0);
        OWNER.set(0);
        let all = complete();
        for partial in [
            FlightFunctions { set: None, ..all },
            FlightFunctions { reset: None, ..all },
            FlightFunctions {
                retire: None,
                ..all
            },
            FlightFunctions {
                status: None,
                ..all
            },
        ] {
            assert_eq!(invoke_set(Some(partial), 17, true), 0);
        }
        assert_eq!(invoke_set(None, 17, true), 0);
        assert_eq!(MUTATIONS.get(), 0);
        assert_eq!(OWNER.get(), 0);
    }
    #[test]
    fn complete_flight_exports_receive_the_host_owner_token() {
        MUTATIONS.set(0);
        OWNER.set(0);
        ON.set(0);
        let owner = (1u64 << 63) | 17;
        assert_eq!(invoke_set(Some(complete()), owner, true), 1);
        assert_eq!(MUTATIONS.get(), 1);
        assert_eq!(OWNER.get(), owner);
        assert_eq!(ON.get(), 1);
        assert_eq!(invoke_set(Some(complete()), 0, false), -1);
        assert_eq!(MUTATIONS.get(), 1);
    }
}
pub fn flight_reset(token: u64) -> i32 {
    if token == 0 {
        return -1;
    }
    let Some(call) = provider().and_then(|p| p.flight_reset) else {
        return 0;
    };
    unsafe { call(token) }
}
pub fn flight_retire(token: u64) -> i32 {
    if token == 0 {
        return -1;
    }
    let Some(call) = provider().and_then(|p| p.flight_retire) else {
        return 0;
    };
    unsafe { call(token) }
}
pub fn flight_status(token: u64) -> Option<FlightClientStatus> {
    if token == 0 {
        return None;
    }
    let call = provider()?.flight_status?;
    let mut status = FlightClientStatus::request();
    let result = unsafe { call(token, &mut status) };
    (result == 1
        && status.struct_bytes == 32
        && status.version == 1
        && status.reserved0 == 0
        && status.reserved1 == 0
        && status.flags & !7 == 0
        && status.mode <= 1)
        .then_some(status)
}
