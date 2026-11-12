use super::provider::provider;
pub fn take_vein_notices() -> Result<crate::VeinNoticeBatch, i32> {
    let Some(call) = provider().and_then(|p| p.take_vein_notices) else {
        return Err(3);
    };
    let mut value = crate::VeinNativeNotices::default();
    let status = unsafe {
        call(
            &mut value,
            std::mem::size_of::<crate::VeinNativeNotices>() as u32,
        )
    };
    if status != 0 {
        return Err(status);
    }
    value.decode()
}
