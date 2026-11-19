#[cfg(windows)]
pub struct QueueWorkerMutex(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for QueueWorkerMutex {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::Threading::ReleaseMutex(self.0);
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
pub fn acquire_queue_worker_mutex(
    root: &std::path::Path,
) -> Result<Option<QueueWorkerMutex>, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::WaitForSingleObject,
    };
    let handle = create_queue_worker_mutex_handle(root)?;
    match unsafe { WaitForSingleObject(handle, 0) } {
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Some(QueueWorkerMutex(handle))),
        WAIT_TIMEOUT => {
            unsafe { CloseHandle(handle) };
            Ok(None)
        }
        result => {
            let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            unsafe { CloseHandle(handle) };
            Err(format!(
                "could not acquire the update queue lock (wait result {result}, Windows error {error})"
            ))
        }
    }
}

#[cfg(windows)]
pub fn wait_for_queue_worker_mutex(root: &std::path::Path) -> Result<QueueWorkerMutex, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{INFINITE, WaitForSingleObject},
    };
    let handle = create_queue_worker_mutex_handle(root)?;
    match unsafe { WaitForSingleObject(handle, INFINITE) } {
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(QueueWorkerMutex(handle)),
        result => {
            let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            unsafe { CloseHandle(handle) };
            Err(format!(
                "could not wait for the update queue lock (wait result {result}, Windows error {error})"
            ))
        }
    }
}

#[cfg(windows)]
fn create_queue_worker_mutex_handle(
    root: &std::path::Path,
) -> Result<windows_sys::Win32::Foundation::HANDLE, String> {
    use sha2::Digest;
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let normalized_root = root
        .canonicalize()
        .map_err(|error| format!("could not resolve installation path for update lock: {error}"))?;
    let name = format!(
        "Local\\ShroudForgeUpdateQueue_{:x}",
        sha2::Sha256::digest(
            normalized_root
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_bytes()
        )
    );
    let wide: Vec<u16> = OsStr::new(&name).encode_wide().chain(Some(0)).collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
    if handle.is_null() {
        return Err(format!(
            "could not create the update queue lock (Windows error {})",
            unsafe { windows_sys::Win32::Foundation::GetLastError() }
        ));
    }
    Ok(handle)
}
