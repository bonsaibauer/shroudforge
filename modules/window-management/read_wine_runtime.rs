use std::sync::OnceLock;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

/// Detect Wine and Proton from the Windows process that owns the window.
pub fn running_under_wine() -> bool {
    static WINE: OnceLock<bool> = OnceLock::new();
    *WINE.get_or_init(|| unsafe {
        let ntdll = GetModuleHandleA(c"ntdll.dll".as_ptr().cast());
        !ntdll.is_null() && GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()).is_some()
    })
}
