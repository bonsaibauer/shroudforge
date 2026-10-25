use super::read_window_focus::{foreground_is_shroudforge_window, foreground_process};
use super::window::UiEvent;
use std::{
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
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
        let mut was_down = false;
        let mut previous_binding = None;
        let mut last_toggle: Option<Instant> = None;
        loop {
            let binding = hotkey_key_watcher
                .read()
                .ok()
                .and_then(|binding| binding.clone());
            let down = binding
                .as_ref()
                .is_some_and(shroudforge_hotkeys::is_hotkey_down);
            if binding != previous_binding {
                was_down = down;
                previous_binding = binding;
            }
            // The first press in game was seen twice, 24 ms apart, while
            // the window took the focus: it opened and closed at once. A
            // press within 250 ms of the last toggle is not a new one.
            let bounced = last_toggle.is_some_and(|at| at.elapsed() < Duration::from_millis(250));
            if down && !was_down && !bounced && !shroudforge_hotkeys::is_capture_active() {
                let foreground = foreground_process();
                let focused = !desktop
                    && (foreground == game_pid
                        || foreground == unsafe { GetCurrentProcessId() }
                        || foreground_is_shroudforge_window());
                if focused {
                    if hotkey_proxy.send_event(UiEvent::Toggle).is_err() {
                        break;
                    }
                    last_toggle = Some(Instant::now());
                }
            }
            was_down = down;
            thread::sleep(Duration::from_millis(5));
        }
    });
}
