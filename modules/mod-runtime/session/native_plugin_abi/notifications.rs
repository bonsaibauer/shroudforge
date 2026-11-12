use std::{ffi::c_void, mem::size_of};
pub(super) const SERVICE: &str = crate::notice_protocol::SERVICE;
pub(super) trait Owner {
    fn owner_thread(&self) -> bool;
    fn revoked(&self) -> bool;
    fn available(&self) -> bool;
    fn package(&self) -> &str;
    fn persist(&self, id: &str, title: &str, message: &str, level: u32) -> i32;
}
type Publish =
    unsafe extern "C" fn(*mut c_void, *const u8, u32, *const u8, u32, *const u8, u32, u32) -> i32;
#[repr(C)]
pub(super) struct NotificationServiceV1 {
    pub size: u32,
    pub version: u32,
    pub context: *mut c_void,
    pub publish: Option<Publish>,
}
pub(super) unsafe fn query_v1<T: Owner>(
    context: *mut c_void,
    version: u32,
    capacity: u32,
    output: *mut c_void,
) -> i32 {
    if context.is_null() {
        return 1;
    }
    let owner = unsafe { &*context.cast::<T>() };
    if !owner.owner_thread() {
        return 8;
    }
    if owner.revoked() || !owner.available() {
        return 3;
    }
    if version != 1 {
        return 2;
    }
    if capacity < size_of::<NotificationServiceV1>() as u32 {
        return 5;
    }
    if output.is_null() {
        return 1;
    }
    unsafe {
        output
            .cast::<NotificationServiceV1>()
            .write(NotificationServiceV1 {
                size: size_of::<NotificationServiceV1>() as u32,
                version: 1,
                context,
                publish: Some(publish::<T>),
            });
    }
    0
}
unsafe extern "C" fn publish<T: Owner>(
    context: *mut c_void,
    id: *const u8,
    id_bytes: u32,
    title: *const u8,
    title_bytes: u32,
    message: *const u8,
    message_bytes: u32,
    level: u32,
) -> i32 {
    if context.is_null() {
        return 1;
    }
    let owner = unsafe { &*context.cast::<T>() };
    if !owner.owner_thread() {
        return 8;
    }
    if owner.revoked() || !owner.available() {
        return 3;
    }
    if id.is_null()
        || title.is_null()
        || message.is_null()
        || id_bytes == 0
        || id_bytes > 80
        || title_bytes == 0
        || title_bytes > 120
        || message_bytes == 0
        || message_bytes > 2000
        || level > 4
    {
        return 1;
    }
    let (Ok(id), Ok(title), Ok(message)) = (
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(id, id_bytes as usize) }),
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(title, title_bytes as usize) }),
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(message, message_bytes as usize) }),
    ) else {
        return 1;
    };
    if !crate::notice_protocol::valid(owner.package(), id, title, message, level) {
        return 1;
    }
    owner.persist(id, title, message, level)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Context {
        thread: Cell<bool>,
        revoked: Cell<bool>,
        allowed: Cell<bool>,
        calls: Cell<u32>,
        result: i32,
    }
    impl Owner for Context {
        fn owner_thread(&self) -> bool {
            self.thread.get()
        }
        fn revoked(&self) -> bool {
            self.revoked.get()
        }
        fn available(&self) -> bool {
            self.allowed.get()
        }
        fn package(&self) -> &str {
            "minimap_mod"
        }
        fn persist(&self, id: &str, title: &str, message: &str, level: u32) -> i32 {
            assert_eq!(
                (id, title, message, level),
                ("visibility", "Minimap", "Minimap On", 0)
            );
            self.calls.set(self.calls.get() + 1);
            self.result
        }
    }
    fn context() -> Context {
        Context {
            thread: Cell::new(true),
            revoked: Cell::new(false),
            allowed: Cell::new(true),
            calls: Cell::new(0),
            result: 0,
        }
    }
    fn query(c: &mut Context) -> NotificationServiceV1 {
        let mut out = std::mem::MaybeUninit::<NotificationServiceV1>::uninit();
        assert_eq!(
            unsafe {
                query_v1::<Context>(
                    (c as *mut Context).cast(),
                    1,
                    size_of::<NotificationServiceV1>() as u32,
                    out.as_mut_ptr().cast(),
                )
            },
            0
        );
        unsafe { out.assume_init() }
    }
    unsafe fn send(s: &NotificationServiceV1, id: &[u8]) -> i32 {
        unsafe {
            s.publish.unwrap()(
                s.context,
                id.as_ptr(),
                id.len() as u32,
                b"Minimap".as_ptr(),
                7,
                b"Minimap On".as_ptr(),
                10,
                0,
            )
        }
    }
    #[test]
    fn notice_service_is_exact_owner_bound_and_optional() {
        assert_eq!(size_of::<NotificationServiceV1>(), 24);
        let mut c = context();
        let s = query(&mut c);
        assert_eq!((s.size, s.version), (24, 1));
        assert_eq!(unsafe { send(&s, b"visibility") }, 0);
        assert_eq!(c.calls.get(), 1);
        c.revoked.set(true);
        assert_eq!(unsafe { send(&s, b"visibility") }, 3);
        assert_eq!(c.calls.get(), 1);
        c.revoked.set(false);
        c.thread.set(false);
        assert_eq!(unsafe { send(&s, b"visibility") }, 8);
    }
    #[test]
    fn notice_service_bounds_roles_and_feed_failure_do_not_invent_success() {
        let mut c = context();
        c.result = 7;
        let s = query(&mut c);
        assert_eq!(unsafe { send(&s, b"visibility") }, 7);
        for id in [b"other/id".as_slice(), b"visibility\0", &[0xff]] {
            assert_eq!(unsafe { send(&s, id) }, 1);
        }
        assert_eq!(c.calls.get(), 1);
        c.allowed.set(false);
        assert_eq!(unsafe { send(&s, b"visibility") }, 3);
        let mut output = [0xa5u8; 24];
        let pointer = (&mut c as *mut Context).cast();
        assert_eq!(
            unsafe { query_v1::<Context>(pointer, 1, 24, output.as_mut_ptr().cast()) },
            3
        );
        assert_eq!(output, [0xa5; 24]);
        c.allowed.set(true);
        assert_eq!(
            unsafe { query_v1::<Context>(pointer, 2, 24, output.as_mut_ptr().cast()) },
            2
        );
        assert_eq!(
            unsafe { query_v1::<Context>(pointer, 1, 23, output.as_mut_ptr().cast()) },
            5
        );
        assert_eq!(output, [0xa5; 24]);
    }
}
