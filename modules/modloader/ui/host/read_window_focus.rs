use super::read_host_arguments::SYNCHRONIZE_ACCESS;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Threading::OpenEventW;
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

pub(crate) fn open_event(name: &str) -> HANDLE {
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    wide.push(0);
    unsafe { OpenEventW(SYNCHRONIZE_ACCESS, 0, wide.as_ptr()) }
}

pub(crate) fn foreground_process() -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
    pid
}

pub(crate) fn foreground_is_shroudforge_window() -> bool {
    foreground_title().starts_with("ShroudForge")
}

/// Title of the foreground window, empty when there is none.
pub(crate) fn foreground_title() -> String {
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return String::new();
    }
    let mut title = [0u16; 256];
    let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&title[..length as usize])
}
