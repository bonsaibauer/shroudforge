use super::read_window_focus::{foreground_is_shroudforge_window, foreground_process};
use super::window::UiEvent;
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};
use tao::event_loop::EventLoopProxy;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

pub(crate) fn watch_toggle_key(
    hotkey_proxy: EventLoopProxy<UiEvent>,
    hotkey_key_watcher: Arc<RwLock<Option<shroudforge_hotkeys::Hotkey>>>,
    game_pid: u32,
    desktop: bool,
) {
    thread::spawn(move || {
        let mut last_press_revision = 0;
        let mut previous_binding = None;
        loop {
            let binding = hotkey_key_watcher
                .read()
                .ok()
                .and_then(|binding| binding.clone());
            let press_revision = binding
                .as_ref()
                .map_or(0, shroudforge_hotkeys::hotkey_press_revision);
            if binding != previous_binding {
                last_press_revision = press_revision;
                previous_binding = binding;
            }
            if press_revision > last_press_revision {
                last_press_revision = press_revision;
                let foreground = foreground_process();
                let focused = !desktop
                    && (foreground == game_pid
                        || foreground == unsafe { GetCurrentProcessId() }
                        || foreground_is_shroudforge_window());
                if focused && !shroudforge_hotkeys::is_capture_active() {
                    if hotkey_proxy.send_event(UiEvent::Toggle).is_err() {
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    });
}
