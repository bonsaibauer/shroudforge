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
        let mut toggle_debouncer = shroudforge_hotkeys::WindowToggleDebouncer::new(0);
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
                toggle_debouncer.reset(press_revision);
                previous_binding = binding;
            }
            if toggle_debouncer.should_toggle(press_revision, || {
                let foreground = foreground_process();
                let focused = !desktop
                    && (foreground == game_pid
                        || foreground == unsafe { GetCurrentProcessId() }
                        || foreground_is_shroudforge_window());
                focused && !shroudforge_hotkeys::is_capture_active()
            }) {
                if hotkey_proxy.send_event(UiEvent::Toggle).is_err() {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    });
}
